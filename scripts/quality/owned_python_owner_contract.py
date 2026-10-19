"""限定したMoon ownerのPython AST形状を検査する。"""

from __future__ import annotations

import ast
import hashlib
import json
import sys
from typing import TypedDict

CONTRACT = "moon-owner-host-evidence-barrier-v3"
LIMITATIONS = [
    "Checks supported AST structure, CLI entry, scheduler barrier assertions, and early returns only.",
    "Does not execute or prove external Moon, Bun, filesystem, or subprocess behavior.",
]


# 実 owner のレビュー済み AST。更新には owner のレビューと実回帰が必要。
OWNER_AST_SHA256 = "5645128a0d92f68f58fea9e9f5d22bb648539eaf7ba25010843f92a682f03b6d"
SCHEDULER_LEAVES = {
    "biome": "biome-run",
    "runtime-asset-script-test": "runtime-asset-script-aggregate-run",
    "runtime-asset-script-other": "runtime-asset-script-other-run",
    "runtime-asset-script-preflight": "runtime-asset-script-preflight-run",
    "runtime-unused-fragment-loader-producer": "runtime-unused-script-producer-run",
    "runtime-unused-bundle-definitions-producer": "runtime-unused-script-producer-run",
    "runtime-unused-check-producer": "runtime-unused-script-producer-run",
    "runtime-unused-tool-fingerprint-producer": "runtime-unused-script-producer-run",
    "runtime-unused-mathjax-producer": "runtime-unused-script-producer-run",
    "runtime-unused-native-width-producer": "runtime-unused-script-producer-run",
    "runtime-unused-rust-classic-producer": "runtime-unused-script-producer-run",
    "runtime-unused-script-regression": "runtime-unused-script-final-verify-run",
}


def _owner_ast_shape(value: object) -> object:
    if isinstance(value, ast.AST):
        return {
            "node": type(value).__name__,
            "fields": {
                name: _owner_ast_shape(child)
                for name, child in ast.iter_fields(value)
                # 3.12 以降の空 type_params だけを 3.9 と同じ形へ正規化する。
                if not (name == "type_params" and child == [])
            },
        }
    if isinstance(value, list):
        return [_owner_ast_shape(child) for child in value]
    return value


def _main_owner_is_supported(source: str) -> bool:
    try:
        normalized = json.dumps(
            _owner_ast_shape(ast.parse(source)),
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
        )
    except (SyntaxError, TypeError, ValueError):
        return False
    return hashlib.sha256(normalized.encode()).hexdigest() == OWNER_AST_SHA256


class SchedulerTask(TypedDict):
    deps: list[str]
    leaf: str | None
    options: dict[str, str | bool]


def _scheduler_tasks(source: str) -> dict[str, SchedulerTask]:
    literals: dict[str, object] = {}
    for node in ast.parse(source).body:
        if (
            isinstance(node, ast.AnnAssign)
            and isinstance(node.target, ast.Name)
            and node.target.id in {"SCHEDULER_DEPENDENCIES", "SCHEDULER_TASK_OPTIONS"}
        ):
            if node.value is not None:
                literals[node.target.id] = ast.literal_eval(node.value)
        elif isinstance(node, ast.Assign) and len(node.targets) == 1:
            target = node.targets[0]
            if isinstance(target, ast.Name) and target.id == "SCHEDULER_NOOP_TASKS":
                literals[target.id] = ast.literal_eval(node.value)
    dependencies = literals.get("SCHEDULER_DEPENDENCIES")
    noops = literals.get("SCHEDULER_NOOP_TASKS")
    options = literals.get("SCHEDULER_TASK_OPTIONS")
    if (
        not isinstance(dependencies, dict)
        or not isinstance(noops, set)
        or not isinstance(options, dict)
    ):
        raise ValueError("reviewed scheduler literals missing")
    typed_options: dict[str, str | bool] = {}
    for key, value in options.items():
        if not isinstance(key, str) or not isinstance(value, (str, bool)):
            raise ValueError("invalid scheduler option literal")
        typed_options[key] = value
    result: dict[str, SchedulerTask] = {}
    for name, deps in dependencies.items():
        if not isinstance(name, str) or not isinstance(deps, list):
            raise ValueError("invalid scheduler dependency literal")
        typed_deps: list[str] = []
        for dep in deps:
            if not isinstance(dep, str):
                raise ValueError("invalid scheduler dependency name")
            typed_deps.append(dep)
        result[name] = {
            "deps": sorted(set(typed_deps)),
            "leaf": None if name in noops else SCHEDULER_LEAVES.get(name, name),
            "options": typed_options.copy(),
        }
    return result


def inspect_owner(source: str) -> dict[str, object]:
    valid = _main_owner_is_supported(source)
    return {
        "schema": 1,
        "contract": CONTRACT,
        "valid": valid,
        "schedulerTasks": _scheduler_tasks(source) if valid else None,
        "limitations": LIMITATIONS,
    }


if __name__ == "__main__":
    print(json.dumps(inspect_owner(sys.stdin.read()), separators=(",", ":")))
