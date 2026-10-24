#!/usr/bin/env python3
"""Regression tests for keeping direct skrifa aligned with the selected usvg."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from typing import TypedDict
from unittest.mock import Mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/release/update_usvg_skrifa_pair.py"
SPEC = importlib.util.spec_from_file_location("update_usvg_skrifa_pair", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
RUNTIME_ID = "path+file:///workspace#katana-render-runtime@0.1.0"
RESVG_ID = f"{REGISTRY}#resvg@0.48.1"
USVG_ID = f"{REGISTRY}#usvg@0.48.1"
OLD_SKRIFA_ID = f"{REGISTRY}#skrifa@0.48.0"
TARGET_SKRIFA_ID = f"{REGISTRY}#skrifa@0.44.0"


class CargoDependencyKind(TypedDict):
    kind: str | None
    target: str | None


class CargoMetadataEdge(TypedDict):
    name: str
    pkg: str
    dep_kinds: list[CargoDependencyKind]


class CargoPackageDependency(TypedDict):
    name: str
    source: str | None
    req: str
    kind: str | None
    optional: bool


class CargoPackage(TypedDict):
    id: str
    name: str
    version: str
    source: str | None
    dependencies: list[CargoPackageDependency]


class CargoResolveNode(TypedDict):
    id: str
    deps: list[CargoMetadataEdge]


class CargoResolve(TypedDict):
    nodes: list[CargoResolveNode]


class CargoMetadata(TypedDict):
    packages: list[CargoPackage]
    workspace_members: list[str]
    resolve: CargoResolve


def edge(name: str, package_id: str) -> CargoMetadataEdge:
    return {
        "name": name,
        "pkg": package_id,
        "dep_kinds": [{"kind": None, "target": None}],
    }


def dependency(
    name: str, requirement: str, source: str | None, kind: str | None = None
) -> CargoPackageDependency:
    return {
        "name": name,
        "source": source,
        "req": requirement,
        "kind": kind,
        "optional": True,
    }


def metadata(
    direct_id: str = TARGET_SKRIFA_ID,
    direct_req: str = "^0.44",
    resvg_name: str = "resvg",
    usvg_name: str = "usvg",
    include_decoy: bool = False,
) -> CargoMetadata:
    resvg_id = f"{REGISTRY}#{resvg_name}@0.48.1"
    usvg_id = f"{REGISTRY}#{usvg_name}@0.48.1"
    versions = {
        OLD_SKRIFA_ID: "0.48.0",
        TARGET_SKRIFA_ID: "0.44.0",
    }
    packages: list[CargoPackage] = [
        {
            "id": RUNTIME_ID,
            "name": "katana-render-runtime",
            "version": "0.1.0",
            "source": None,
            "dependencies": [dependency("skrifa", direct_req, REGISTRY)],
        },
        {
            "id": resvg_id,
            "name": resvg_name,
            "version": "0.48.1",
            "source": REGISTRY,
            "dependencies": [],
        },
        {
            "id": usvg_id,
            "name": usvg_name,
            "version": "0.48.1",
            "source": REGISTRY,
            "dependencies": [dependency("skrifa", "^0.44", REGISTRY)],
        },
    ]
    packages.extend(
        {
            "id": package_id,
            "name": "skrifa",
            "version": version,
            "source": REGISTRY,
            "dependencies": [],
        }
        for package_id, version in versions.items()
    )
    runtime_deps: list[CargoMetadataEdge] = [
        edge("resvg", resvg_id),
        edge("skrifa", direct_id),
    ]
    nodes: list[CargoResolveNode] = [
        {"id": RUNTIME_ID, "deps": runtime_deps},
        {"id": resvg_id, "deps": [edge("usvg", usvg_id)]},
        {"id": usvg_id, "deps": [edge("skrifa", TARGET_SKRIFA_ID)]},
        {"id": OLD_SKRIFA_ID, "deps": []},
        {"id": TARGET_SKRIFA_ID, "deps": []},
    ]
    if include_decoy:
        helper_id = f"{REGISTRY}#metadata-helper@0.1.0"
        decoy_id = f"{REGISTRY}#usvg@0.49.0"
        packages.extend(
            [
                {
                    "id": helper_id,
                    "name": "metadata-helper",
                    "version": "0.1.0",
                    "source": REGISTRY,
                    "dependencies": [],
                },
                {
                    "id": decoy_id,
                    "name": "usvg",
                    "version": "0.49.0",
                    "source": REGISTRY,
                    "dependencies": [],
                },
            ]
        )
        runtime_deps.append(edge("metadata-helper", helper_id))
        nodes.extend(
            [
                {"id": helper_id, "deps": [edge("usvg", decoy_id)]},
                {"id": decoy_id, "deps": []},
            ]
        )
    return {
        "packages": packages,
        "workspace_members": [RUNTIME_ID],
        "resolve": {"nodes": nodes},
    }


class UpdateUsvgSkrifaPairTest(unittest.TestCase):
    def test_wrapper_command_is_split_without_shell_execution(self) -> None:
        self.assertEqual(MODULE.parse_cargo_command("/opt/homebrew/bin/rtk cargo"), ["/opt/homebrew/bin/rtk", "cargo"])
        self.assertEqual(MODULE.parse_cargo_command("cargo; touch /tmp/no-shell"), ["cargo;", "touch", "/tmp/no-shell"])

    def test_omitted_caret_requirement_keeps_manifest_bytes(self) -> None:
        contents = '[dependencies]\nskrifa = "0.44" # selected policy\n'
        updated, changed = MODULE.sync_manifest_text(contents, "^0.44")
        self.assertFalse(changed)
        self.assertEqual(updated, contents)

    def test_sync_changes_only_the_direct_string_requirement(self) -> None:
        contents = '[dependencies]\nanyhow = "1"\nskrifa = "0.44" # cmap policy\n\n[dev-dependencies]\nskrifa = "0.48"\n'
        updated, changed = MODULE.sync_manifest_text(contents, "^0.45")
        self.assertTrue(changed)
        self.assertEqual(
            updated,
            '[dependencies]\nanyhow = "1"\nskrifa = "^0.45" # cmap policy\n\n[dev-dependencies]\nskrifa = "0.48"\n',
        )

    def test_malformed_manifest_fails_with_toml_error(self) -> None:
        with self.assertRaisesRegex(ValueError, "cannot parse KRR Cargo.toml"):
            MODULE.sync_manifest_text('[dependencies\nskrifa = "0.44"\n', "^0.44")

    def test_selects_only_usvg_reachable_from_krr(self) -> None:
        runtime, _, selected, _ = MODULE.select_usvg_package(metadata())
        self.assertEqual(runtime["id"], RUNTIME_ID)
        self.assertEqual(selected["id"], USVG_ID)

    def test_selects_fork_packages_through_logical_edges_with_public_decoy(self) -> None:
        graph = metadata(
            resvg_name="katana-resvg",
            usvg_name="katana-usvg",
            include_decoy=True,
        )
        _, _, selected, _ = MODULE.select_usvg_package(graph)
        self.assertEqual(selected["name"], "katana-usvg")

    def test_reachable_dangling_dependency_fails_closed(self) -> None:
        graph = metadata()
        graph["resolve"]["nodes"][0]["deps"].append(edge("broken", "missing-package"))
        with self.assertRaisesRegex(ValueError, "missing package node"):
            MODULE.select_usvg_package(graph)

    def test_ambiguous_usvg_resolution_fails_closed(self) -> None:
        graph = metadata()
        packages = graph["packages"]
        nodes = graph["resolve"]["nodes"]
        packages.append(
            {
                "id": f"{REGISTRY}#usvg@0.49.0",
                "name": "usvg",
                "version": "0.49.0",
                "source": REGISTRY,
                "dependencies": [],
            }
        )
        nodes.append({"id": f"{REGISTRY}#usvg@0.49.0", "deps": []})
        nodes[1]["deps"].append(edge("usvg", f"{REGISTRY}#usvg@0.49.0"))
        with self.assertRaisesRegex(ValueError, "expected one resolved usvg"):
            MODULE.select_usvg_package(graph)

    def test_non_registry_usvg_dependency_fails_closed(self) -> None:
        selected = next(package for package in metadata()["packages"] if package["id"] == USVG_ID)
        selected["dependencies"][0]["source"] = "registry+https://example.invalid/index"
        with self.assertRaisesRegex(ValueError, "different registry"):
            MODULE.skrifa_requirement(selected)

    def test_unaligned_resolved_versions_fail_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "different package versions"):
            MODULE.validate_resolution(metadata(OLD_SKRIFA_ID, "^0.48"), "^0.44")

    def test_runner_updates_requirement_then_checks_resolved_identity(self) -> None:
        before = metadata(OLD_SKRIFA_ID, "^0.48")
        after = metadata(TARGET_SKRIFA_ID, "^0.44")
        responses = [json.dumps(before), json.dumps(after)]

        def runner(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            self.assertEqual(kwargs["cwd"], MODULE.ROOT)
            if command[-3:] == ["metadata", "--format-version", "1"]:
                return subprocess.CompletedProcess(command, 0, stdout=responses.pop(0))
            self.assertEqual(command[-3:], ["update", "-p", "skrifa@0.48.0"])
            return subprocess.CompletedProcess(command, 0)

        with tempfile.TemporaryDirectory() as directory:
            manifest_path = Path(directory) / "Cargo.toml"
            manifest_path.write_text('[dependencies]\nskrifa = "0.48"\n', encoding="utf-8")
            MODULE.run_update(["rtk", "cargo"], runner, manifest_path)
            self.assertEqual(manifest_path.read_text(encoding="utf-8"), '[dependencies]\nskrifa = "^0.44"\n')
        self.assertEqual(responses, [])

    def test_already_aligned_requirement_does_not_rewrite_or_update(self) -> None:
        result = subprocess.CompletedProcess([], 0, stdout=json.dumps(metadata()))
        runner = Mock(return_value=result)
        with tempfile.TemporaryDirectory() as directory:
            manifest_path = Path(directory) / "Cargo.toml"
            original = '[dependencies]\nskrifa = "0.44"\n'
            manifest_path.write_text(original, encoding="utf-8")
            MODULE.run_update(["cargo"], runner, manifest_path)
            self.assertEqual(manifest_path.read_text(encoding="utf-8"), original)
        self.assertEqual(runner.call_count, 2)
        self.assertTrue(all("update" not in call.args[0] for call in runner.call_args_list))

    def test_depends_update_all_orders_usvg_pair_and_harness_lock_before_bun(self) -> None:
        justfile = (ROOT / "Justfile").read_text(encoding="utf-8")
        recipe = justfile.split("depends-update-mutations:\n", maxsplit=1)[1].split("\n\n", maxsplit=1)[0]
        commands = [line.strip() for line in recipe.splitlines() if line.strip()]
        broad_upgrade = "{{CARGO}} upgrade -i allow --pinned allow --exclude skrifa"
        html_pair = "python3 scripts/release/update_html5ever_pair.py --cargo \"{{CARGO}}\""
        lock_update = "{{CARGO}} update"
        skrifa_pair = "python3 scripts/release/update_usvg_skrifa_pair.py --cargo \"{{CARGO}}\""
        harness_lock = "python3 scripts/release/update_html_performance_harness_lock.py --cargo \"{{CARGO}}\""
        bun_update = "bun update --latest '!@moonrepo/*'"

        for command in (broad_upgrade, html_pair, lock_update, skrifa_pair, harness_lock, bun_update):
            self.assertEqual(commands.count(command), 1, f"expected one {command!r}")
        broad_index = commands.index(broad_upgrade)
        html_index = commands.index(html_pair)
        lock_index = commands.index(lock_update)
        skrifa_index = commands.index(skrifa_pair)
        harness_index = commands.index(harness_lock)
        bun_update_index = commands.index(bun_update)
        self.assertLess(broad_index, html_index)
        self.assertLess(html_index, lock_index)
        self.assertLess(lock_index, skrifa_index)
        self.assertLess(skrifa_index, harness_index)
        self.assertLess(skrifa_index, bun_update_index)
        self.assertLess(harness_index, bun_update_index)


if __name__ == "__main__":
    unittest.main()
