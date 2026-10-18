#!/usr/bin/env python3
"""Classify repository-owned Python sources without importing or executing them."""
from __future__ import annotations

import argparse
import ast
import json
import sys
from pathlib import Path
from typing import Literal, TypedDict

Kind = Literal["unittest", "none", "unknown"]


class InventoryRecord(TypedDict):
    path: str
    kind: Kind


def _is_testcase_reference(node: ast.expr, unittest_names: set[str], testcase_names: set[str]) -> bool:
    if isinstance(node, ast.Name):
        return node.id in testcase_names
    return (
        isinstance(node, ast.Attribute)
        and isinstance(node.value, ast.Name)
        and node.value.id in unittest_names
        and node.attr == "TestCase"
    )


def _looks_like_unsupported_testcase(node: ast.expr) -> bool:
    if isinstance(node, ast.Name):
        return node.id.endswith("TestCase") or node.id.endswith("Test")
    if isinstance(node, ast.Attribute):
        return node.attr in {"TestCase", "TestCaseMixin"} or node.attr.endswith("TestCase")
    return isinstance(node, (ast.Call, ast.Subscript))


def classify_python_source(source: str) -> Kind:
    """Classify source from its AST; no source code is imported or executed."""
    tree = ast.parse(source)
    unittest_names: set[str] = set()
    testcase_names: set[str] = set()
    shadowed_names: set[str] = set()

    for statement in tree.body:
        if isinstance(statement, ast.Import):
            for alias in statement.names:
                bound = alias.asname or alias.name.split(".", 1)[0]
                if alias.name == "unittest" and bound not in unittest_names:
                    unittest_names.add(bound)
        elif isinstance(statement, ast.ImportFrom) and statement.module == "unittest":
            for alias in statement.names:
                bound = alias.asname or alias.name
                if alias.name == "TestCase" and bound not in testcase_names:
                    testcase_names.add(bound)

    imported_names = unittest_names | testcase_names
    seen_import_bindings: set[str] = set()
    for statement in tree.body:
        names = _statement_bound_names(statement)
        if isinstance(statement, (ast.Import, ast.ImportFrom)):
            for name in names & imported_names:
                if name in seen_import_bindings:
                    shadowed_names.add(name)
                seen_import_bindings.add(name)
        else:
            shadowed_names.update(names & imported_names)
        if _mutates_unittest_attribute(statement, unittest_names):
            shadowed_names.update(unittest_names)

    if shadowed_names:
        return "unknown"

    classes = [statement for statement in tree.body if isinstance(statement, ast.ClassDef)]
    known_local: set[str] = set()
    recognized = False
    unsupported = False
    for class_node in classes:
        recognized_base = False
        unknown_base = False
        for base in class_node.bases:
            local_base = isinstance(base, ast.Name) and base.id in known_local
            if _is_testcase_reference(base, unittest_names, testcase_names) or local_base:
                recognized_base = True
            elif not (isinstance(base, ast.Name) and base.id == "object"):
                unknown_base = True
                unsupported = unsupported or _looks_like_unsupported_testcase(base)
        if recognized_base:
            known_local.add(class_node.name)
            recognized = True
            unsupported = unsupported or unknown_base

    rebound_local: set[str] = set()
    seen_classes: set[str] = set()
    for statement in tree.body:
        names = _statement_bound_names(statement) & known_local
        if isinstance(statement, ast.ClassDef):
            rebound_local.update(names & seen_classes)
            seen_classes.update(names)
        else:
            rebound_local.update(names)
        if _mutates_unittest_attribute(statement, known_local, {"__bases__", "__class__", "__mro_entries__"}):
            rebound_local.update(known_local)
    if unsupported or rebound_local:
        return "unknown"
    return "unittest" if recognized else "none"


def _statement_bound_names(statement: ast.stmt) -> set[str]:
    names: set[str] = set()
    if isinstance(statement, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
        names.add(statement.name)
    elif isinstance(statement, (ast.Assign, ast.AnnAssign, ast.AugAssign)):
        targets = statement.targets if isinstance(statement, ast.Assign) else [statement.target]
        for target in targets:
            names.update(_bound_names(target))
    elif isinstance(statement, (ast.For, ast.AsyncFor)):
        names.update(_bound_names(statement.target))
    elif isinstance(statement, (ast.With, ast.AsyncWith)):
        for item in statement.items:
            if item.optional_vars is not None:
                names.update(_bound_names(item.optional_vars))
    elif isinstance(statement, ast.Delete):
        for target in statement.targets:
            names.update(_bound_names(target))
    elif isinstance(statement, ast.Import):
        names.update(alias.asname or alias.name.split(".", 1)[0] for alias in statement.names)
    elif isinstance(statement, ast.ImportFrom):
        names.update(alias.asname or alias.name for alias in statement.names)
    return names


def _bound_names(node: ast.expr) -> set[str]:
    if isinstance(node, ast.Name):
        return {node.id}
    if isinstance(node, (ast.Tuple, ast.List)):
        names: set[str] = set()
        for element in node.elts:
            names.update(_bound_names(element))
        return names
    return set()


def _mutates_unittest_attribute(
    statement: ast.stmt, unittest_names: set[str], attributes: set[str] | None = None,
) -> bool:
    targets: list[ast.expr] = []
    if isinstance(statement, (ast.Assign, ast.AnnAssign, ast.AugAssign)):
        targets.extend(statement.targets if isinstance(statement, ast.Assign) else [statement.target])
    elif isinstance(statement, ast.Delete):
        targets.extend(statement.targets)
    return any(
        isinstance(target, ast.Attribute)
        and isinstance(target.value, ast.Name)
        and target.value.id in unittest_names
        and target.attr in (attributes if attributes is not None else {"TestCase"})
        for target in targets
    )


def _validated_root(value: str) -> Path:
    root = Path(value)
    if not root.is_absolute():
        raise ValueError("--root must be an absolute path")
    if not root.is_dir() or root.is_symlink():
        raise ValueError("--root must be a real directory")
    resolved = root.resolve(strict=True)
    return resolved


def _validated_source(root: Path, relative: str) -> Path:
    if not relative or "\x00" in relative or "\\" in relative:
        raise ValueError("source paths must be non-empty repository-relative paths")
    candidate = Path(relative)
    if candidate.is_absolute() or any(part in {"", ".", ".."} for part in relative.split("/")):
        raise ValueError(f"unsafe repository-relative path: {relative}")
    absolute = root.joinpath(candidate)
    if not absolute.is_file():
        raise ValueError(f"source is not a regular file: {relative}")
    current = root
    for part in candidate.parts:
        current = current / part
        if current.is_symlink():
            raise ValueError(f"source path contains a symlink: {relative}")
    if absolute.resolve(strict=True) != absolute:
        raise ValueError(f"source path resolves outside root: {relative}")
    return absolute


def inventory_paths(root: Path, paths: object) -> list[InventoryRecord]:
    if not isinstance(paths, list):
        raise ValueError("stdin must contain a JSON array of strings")
    records: list[InventoryRecord] = []
    for relative in paths:
        if not isinstance(relative, str):
            raise ValueError("stdin must contain a JSON array of strings")
        source = _validated_source(root, relative)
        kind = classify_python_source(source.read_text(encoding="utf-8"))
        records.append({"path": relative, "kind": kind})
    return records


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True)
    args = parser.parse_args()
    try:
        root = _validated_root(args.root)
        payload = json.load(sys.stdin)
        records = inventory_paths(root, payload)
    except (OSError, UnicodeError, SyntaxError, ValueError, json.JSONDecodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    json.dump(records, sys.stdout, separators=(",", ":"))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
