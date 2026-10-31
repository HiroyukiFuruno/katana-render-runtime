"""列挙結果だけで再利用を許可せず、identity側の採用判定と境界を保つ。"""
from __future__ import annotations

import hashlib
import json
import os
import stat
import subprocess
import sys
from collections.abc import Mapping
from pathlib import Path

import html_performance_inventory as physical_inventory

SCHEMA = "krr-tool-fingerprint-input-diagnostic-v1"
PROFILE = "runtime-unused-tool-fingerprint"
SEMANTIC_SCOPE = "krr-tool-fingerprint-profile-inputs-v1"
ORCHESTRATION_METADATA = frozenset((
    "MOON_CACHE_DIR", "MOON_PROJECT_ID", "MOON_PROJECT_ROOT", "MOON_PROJECT_SOURCE",
    "MOON_PROJECT_SNAPSHOT", "MOON_TARGET", "MOON_TASK_ID", "MOON_TASK_HASH",
    "MOON_TASK_RETRY_ATTEMPT", "MOON_TASK_RETRY_TOTAL",
    "MOON_WORKSPACE_ROOT", "MOON_WORKING_DIR", "MOON_CACHE",
    "MOON_CACHE_CAS_VERIFY_INTEGRITY", "MOON_EXPERIMENT_CAS_OUTPUTS_CACHE",
    "CARGO_CAPACITY_PROTECTED_REFS", "KRR_RUNTIME_CAS_OPERATION",
))
SOURCE_FILES = (
    "scripts/runtime-bundles/runtime-bundle-tool-fingerprint_test.ts",
    "scripts/runtime-bundles/runtime-bundle-tool-fingerprint.ts",
    "scripts/runtime-bundles/runtime-tool-fingerprint-input-probe.ts",
    "package.json",
    "bun.lock",
    "tsconfig.json",
    "Justfile",
    "moon.yml",
    "scripts/mermaid/native-source-manifest.ts",
    "scripts/mermaid/native-build-receipt.ts",
    "scripts/mermaid/mermaid-pair-manifest.ts",
)
OVERRIDES = ("NODE_OPTIONS", "NODE_PATH", "BUN_OPTIONS", "BUN_ENV", "BASH_ENV", "ENV")
ALLOWED_RUNTIME_NAMES = frozenset(("BUN_CONFIG_NO_CLEAR_TERMINAL", "NODE_ENV",
                                   "NODE_REPL_TRUSTED_BROWSER_CLIENT_SHA256S"))
PROBE = "scripts/runtime-bundles/runtime-tool-fingerprint-input-probe.ts"
REVIEWED_SOURCES = {
    SOURCE_FILES[0]: "3e4c69ecb98e779747d44be28795edc5e28ae10621bf319129e8b690691138d7",
    SOURCE_FILES[1]: "b2f734840251199f02372fec8201510276811d880e9976ad8910438c4b52768c",
    SOURCE_FILES[2]: "5a27a1ae123f3668751caf1a35180a1266798a2329db57225693c5ac41523612",
}


def _digest(value: object) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def _config_absence(root: Path, user_home: Path, environment: Mapping[str, str]) -> list[str]:
    candidates = {parent / "bunfig.toml" for parent in (root, *root.parents)}
    xdg = environment.get("XDG_CONFIG_HOME")
    if xdg and not Path(xdg).is_absolute():
        raise ValueError("xdg_config_home_not_absolute")
    candidates.add((Path(xdg) if xdg else user_home) / ".bunfig.toml")
    candidates.update(root / name for name in (
        ".env", ".env.local", ".env.test", ".env.test.local",
        ".env.development", ".env.development.local", ".env.production", ".env.production.local",
    ))
    for path in candidates:
        if os.path.lexists(path):
            raise ValueError("configuration_or_preload_not_admitted")
    return sorted(str(path) for path in candidates)


def _checked_inventory(directory: Path, allowed_root: Path) -> dict[str, object]:
    if directory.is_symlink() or not directory.is_dir():
        raise ValueError("package_directory_not_regular")
    canonical = directory.resolve(strict=True)
    if not canonical.is_relative_to(allowed_root) or canonical != directory:
        raise ValueError("package_directory_outside_root")
    directories_before = _directory_stamps(directory)
    before = physical_inventory.inventory(directory)
    rows = before.get("files")
    if not isinstance(rows, list) or not rows:
        raise ValueError("package_inventory_empty")
    if any(not isinstance(row, dict) or "link" in row for row in rows):
        raise ValueError("package_symlink_not_admitted")
    if physical_inventory.inventory(directory) != before or _directory_stamps(directory) != directories_before:
        raise ValueError("package_inventory_changed")
    return {"files": before, "directories": directories_before}


def _directory_stamps(root: Path) -> list[tuple[str, tuple[int, int, int, int, int, int]]]:
    records: list[tuple[str, tuple[int, int, int, int, int, int]]] = []
    def reject_walk_error(error: OSError) -> None:
        raise error

    for directory, names, _ in os.walk(root, followlinks=False, onerror=reject_walk_error):
        path = Path(directory)
        if any((path / name).is_symlink() for name in names):
            raise ValueError("package_symlink_not_admitted")
        stamp = physical_inventory._stamp(path)
        if not stat.S_ISDIR(stamp[2]):
            raise ValueError("package_directory_not_regular")
        records.append((str(path.relative_to(root)), stamp))
    return sorted(records)


def _regular_record(root: Path, relative: str) -> dict[str, object]:
    path = root / relative
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or path.resolve(strict=True) != path:
        raise ValueError("source_file_not_regular")
    row = physical_inventory._file_row(path, root)
    return row


def _adapter_inventory(repo: Path) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    for relative, pattern in (("scripts/release", "*.py"), ("scripts/mermaid", "*.ts")):
        directory = repo / relative
        names = sorted(path.name for path in directory.glob(pattern))
        if not names:
            raise ValueError("adapter_inventory_empty")
        rows.extend(_regular_record(repo, f"{relative}/{name}") for name in names)
        if names != sorted(path.name for path in directory.glob(pattern)):
            raise ValueError("adapter_inventory_changed")
    return rows


def collect(repo: Path, environment: Mapping[str, str]) -> dict[str, object]:
    """入力列挙に採用権限を持たせず、再利用判定との分離を維持する。"""
    result: dict[str, object] = {"schema": SCHEMA, "profile": PROFILE, "cacheable": False,
                                 "admission": "candidate-only"}
    try:
        if not repo.is_absolute() or repo.resolve(strict=True) != repo:
            raise ValueError("repository_not_canonical")
        unknown_runtime = [name for name in environment if (name.startswith("BUN_") or
                           name.startswith("NODE_")) and name not in ALLOWED_RUNTIME_NAMES]
        if any(environment.get(name) for name in OVERRIDES) or unknown_runtime:
            raise ValueError("unsupported_override")
        if any(value and (name.startswith("LD_") or name.startswith("DYLD_"))
               for name, value in environment.items()):
            raise ValueError("unsupported_loader_override")
        if environment.get("NODE_ENV", "") not in ("", "test", "development", "production"):
            raise ValueError("unsupported_node_env_mode")
        home = Path(environment.get("HOME", ""))
        if not home.is_absolute() or home.resolve(strict=True) != home:
            raise ValueError("home_not_canonical")
        absence = _config_absence(repo, home, environment)
        source_before = [_regular_record(repo, item) for item in SOURCE_FILES]
        if any(source_before[index].get("sha256") != expected
               for index, expected in enumerate(REVIEWED_SOURCES.values())):
            raise ValueError("reviewed_source_changed")
        result_process = subprocess.run(
            ["bun", PROBE], cwd=repo, env=dict(environment),
            capture_output=True, check=False,
        )
        if result_process.returncode != 0:
            raise ValueError("resolver_unavailable")
        resolved = json.loads(result_process.stdout)
        if not isinstance(resolved, dict) or set(resolved) != {
            "zod", "utils", "cli", "moon", "libc", "bun", "platform", "arch"
        }:
            raise ValueError("resolver_shape_unsupported")
        if not isinstance(resolved["platform"], str) or not isinstance(resolved["arch"], str):
            raise ValueError("platform_shape_unsupported")
        if not isinstance(resolved["bun"], str) or not Path(resolved["bun"]).is_absolute():
            raise ValueError("bun_path_invalid")
        bun = Path(resolved["bun"]).resolve(strict=True)
        bun_record = physical_inventory._file_row(bun, bun.parent)
        node_modules = (repo / "node_modules").resolve(strict=True)
        if not node_modules.is_relative_to(repo):
            raise ValueError("node_modules_outside_repository")
        packages = [node_modules / "zod", node_modules / "@moonrepo" / "cli"]
        for key in ("zod", "utils", "cli", "moon"):
            value = resolved[key]
            if not isinstance(value, str) or not Path(value).is_absolute():
                raise ValueError("resolver_path_invalid")
            actual = Path(value).resolve(strict=True)
            if not actual.is_relative_to(node_modules) or not actual.is_file():
                raise ValueError("resolver_outside_installed_packages")
            if key == "zod" and not actual.is_relative_to(packages[0]):
                raise ValueError("zod_resolver_outside_package")
            if key in ("utils", "cli") and not actual.is_relative_to(packages[1]):
                raise ValueError("moon_resolver_outside_package")
            if key == "moon":
                if not actual.parent.name.startswith("core-") or actual.parent.parent != node_modules / "@moonrepo":
                    raise ValueError("moon_platform_package_invalid")
                if not (actual.parent / "package.json").is_file():
                    raise ValueError("moon_platform_manifest_missing")
                packages.append(actual.parent)
        if (sys.platform == "linux") != (resolved["libc"] is not None):
            raise ValueError("libc_platform_mismatch")
        if resolved["libc"] is not None:
            if not isinstance(resolved["libc"], str):
                raise ValueError("libc_resolver_invalid")
            libc = Path(resolved["libc"]).resolve(strict=True)
            if not libc.is_relative_to(node_modules / "detect-libc"):
                raise ValueError("libc_resolver_outside_package")
            packages.append(node_modules / "detect-libc")
        inventories = [_checked_inventory(path, node_modules) for path in packages]
        adapter_inventory = _adapter_inventory(repo)
        if source_before != [_regular_record(repo, item) for item in SOURCE_FILES]:
            raise ValueError("source_changed_during_inventory")
        if absence != _config_absence(repo, home, environment):
            raise ValueError("configuration_changed_during_inventory")
        repeated = subprocess.run(["bun", PROBE], cwd=repo, env=dict(environment),
                                  capture_output=True, check=False)
        if repeated.returncode != 0 or json.loads(repeated.stdout) != resolved:
            raise ValueError("resolver_changed_during_inventory")
        if (source_before != [_regular_record(repo, item) for item in SOURCE_FILES]
                or inventories != [_checked_inventory(path, node_modules) for path in packages]
                or adapter_inventory != _adapter_inventory(repo)
                or absence != _config_absence(repo, home, environment)):
            raise ValueError("inputs_changed_during_resolver")
        semantic_environment = {key: value for key, value in environment.items()
                                if key not in ORCHESTRATION_METADATA}
        components = {"sources": _digest(source_before), "packages": _digest(inventories),
                      "resolver": _digest(resolved), "bun": _digest(bun_record),
                      "configurationAbsence": _digest(absence), "adapter": _digest(adapter_inventory),
                      "semanticEnvironment": _digest(semantic_environment)}
        result.update({"inventorySha256": _digest({"sources": source_before, "packages": inventories,
                                                   "resolved": resolved, "bun": bun_record, "absent": absence,
                                                   "adapter": adapter_inventory, "scope": SEMANTIC_SCOPE,
                                                   "environmentSha256": components["semanticEnvironment"]}),
                       "componentDigests": components,
                       "semanticEnvironmentNames": sorted(semantic_environment),
                       "inputsCollected": True, "missReason": "diagnostic_not_admitted"})
    except ValueError as error:
        reason = str(error)
        result.update({"inputsCollected": False, "missReason": reason if reason.isidentifier()
                       else "input_collection_rejected"})
    except (OSError, TypeError):
        result.update({"inputsCollected": False, "missReason": "input_collection_unavailable"})
    return result
