"""列挙結果だけで再利用を許可せず、identity側の採用判定と境界を保つ。"""
from __future__ import annotations

import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
from collections.abc import Mapping
from pathlib import Path

import html_performance_inventory as physical_inventory
import runtime_script_profiles as profiles

SCHEMA = "krr-tool-fingerprint-input-diagnostic-v1"
PROFILE = "runtime-unused-tool-fingerprint"
SEMANTIC_SCOPE = "krr-tool-fingerprint-profile-inputs-v1"
# MOON_COLORはCLI出力の色だけを制御し、task inputを変えない。
ORCHESTRATION_METADATA = frozenset((
    "MOON_CACHE_DIR", "MOON_COLOR", "MOON_PROJECT_ID", "MOON_PROJECT_ROOT", "MOON_PROJECT_SOURCE",
    "MOON_PROJECT_SNAPSHOT", "MOON_TARGET", "MOON_TASK_ID", "MOON_TASK_HASH",
    "MOON_TASK_RETRY_ATTEMPT", "MOON_TASK_RETRY_TOTAL",
    "MOON_WORKSPACE_ROOT", "MOON_WORKING_DIR", "MOON_CACHE",
    "MOON_CACHE_CAS_VERIFY_INTEGRITY", "MOON_EXPERIMENT_CAS_OUTPUTS_CACHE",
    "CARGO_CAPACITY_PROTECTED_REFS", "KRR_RUNTIME_CAS_OPERATION",
    # 検証ランチャーの一時出力先の所有管理に使う実行単位の識別子。
    "WORKSPACE_RUST_CLEANUP_SCOPE",
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

QUALITY_PROFILES = frozenset(profiles.QUALITY_SCRIPT_PROFILES)


def _quality_ruff_absence(repo: Path, home: Path, environment: Mapping[str, str]) -> list[str]:
    temporary = {Path(tempfile.gettempdir()).resolve(strict=True)}
    for name in ("TMPDIR", "TMP", "TEMP"):
        value = environment.get(name)
        if value:
            if not Path(value).is_absolute():
                raise ValueError("quality_temp_path_not_absolute")
            temporary.add(Path(value).resolve(strict=True))
    directories = {repo.parent, *repo.parents}
    for parent in temporary:
        directories.update((parent, *parent.parents))
    xdg = environment.get("XDG_CONFIG_HOME")
    if xdg and not Path(xdg).is_absolute():
        raise ValueError("quality_xdg_path_not_absolute")
    directories.add((Path(xdg) if xdg else home / ".config") / "ruff")
    candidates = sorted(str(directory / name) for directory in directories
                        for name in ("ruff.toml", ".ruff.toml", "pyproject.toml"))
    if any(os.path.lexists(path) for path in candidates):
        raise ValueError("quality_external_ruff_config_not_admitted")
    return candidates


def _quality_git(repo: Path, environment: Mapping[str, str]) -> str:
    executable = shutil.which("git", path=environment.get("PATH", ""))
    if executable is None:
        raise ValueError("quality_git_unavailable")
    git = Path(executable).resolve(strict=True)
    binary = physical_inventory._file_row(git, git.parent)
    result = subprocess.run(
        [str(git), "config", "--includes", "--null", "--list", "--show-origin"],
        cwd=repo, env=dict(environment), capture_output=True, check=False,
    )
    if result.returncode != 0:
        raise ValueError("quality_git_config_unavailable")
    _quality_git_conditions(git, repo, environment)
    files = _quality_git_files(git, repo, environment)
    return _digest({"binary": binary, "configurationSha256": hashlib.sha256(result.stdout).hexdigest(),
                    "files": files})


def _quality_git_conditions(git: Path, repo: Path, environment: Mapping[str, str]) -> None:
    result = subprocess.run(
        [str(git), "config", "--includes", "--null", "--name-only", "--get-regexp", r"^includeif\..*\.path$"],
        cwd=repo, env=dict(environment), capture_output=True, check=False,
    )
    if result.returncode == 1 and not result.stdout:
        return
    if result.returncode != 0 or not result.stdout.endswith(b"\0"):
        raise ValueError("quality_git_config_unavailable")
    roots = _quality_temp_roots(environment)
    for raw in result.stdout[:-1].split(b"\0"):
        _quality_git_condition(os.fsdecode(raw), roots, environment)


def _quality_temp_roots(environment: Mapping[str, str]) -> set[Path]:
    roots = {Path(tempfile.gettempdir())}
    if os.name != "nt":
        roots.update((Path("/tmp"), Path("/var/tmp")))
    for name in ("TMPDIR", "TMP", "TEMP"):
        if environment.get(name):
            roots.add(Path(environment[name]))
    if any(not root.is_absolute() for root in roots):
        raise ValueError("quality_git_conditional_include_not_admitted")
    return roots | {root.resolve(strict=False) for root in roots}


def _quality_git_condition(name: str, roots: set[Path], environment: Mapping[str, str]) -> None:
    lower = name.lower()
    if not lower.startswith("includeif.") or not lower.endswith(".path"):
        raise ValueError("quality_git_conditional_include_not_admitted")
    condition = name[len("includeif."):-len(".path")]
    if condition.lower().startswith("gitdir/i:"):
        pattern = condition[len("gitdir/i:"):]
    elif condition.lower().startswith("gitdir:"):
        pattern = condition[len("gitdir:"):]
    else:
        raise ValueError("quality_git_conditional_include_not_admitted")
    if "\\" in pattern or any(p in (".", "..") for p in pattern.split("/")):
        raise ValueError("quality_git_conditional_include_not_admitted")
    if pattern.startswith("~/"):
        pattern = (Path(environment.get("HOME", "")) / pattern[2:]).as_posix()
    if not Path(pattern).is_absolute():
        raise ValueError("quality_git_conditional_include_not_admitted")
    first_glob = min((pattern.find(char) for char in "*?[" if char in pattern), default=len(pattern))
    literal = pattern[:first_glob]
    prefix = Path(literal if literal.endswith("/") else literal.rpartition("/")[0] + "/")
    prefixes = (prefix, prefix.resolve(strict=False))
    for left in prefixes:
        for right in roots:
            a, b = left.as_posix().casefold().rstrip("/"), right.as_posix().casefold().rstrip("/")
            if not a or not b or a == b or a.startswith(b + "/") or b.startswith(a + "/"):
                raise ValueError("quality_git_conditional_include_not_admitted")


def _quality_git_files(git: Path, repo: Path, environment: Mapping[str, str]) -> list[object]:
    home = Path(environment.get("HOME", ""))
    xdg = Path(environment.get("XDG_CONFIG_HOME") or home / ".config")
    if not home.is_absolute() or not xdg.is_absolute():
        raise ValueError("quality_git_path_not_absolute")
    paths = {xdg / "git" / "ignore", xdg / "git" / "attributes"}
    for key in ("core.excludesFile", "core.attributesFile"):
        result = subprocess.run([str(git), "config", "--includes", "--null", "--path", "--get-all", key],
                                cwd=repo, env=dict(environment), capture_output=True, check=False)
        if result.returncode not in (0, 1) or (result.returncode == 1 and result.stdout):
            raise ValueError("quality_git_config_unavailable")
        if result.returncode == 0:
            if not result.stdout.endswith(b"\0"):
                raise ValueError("quality_git_config_unavailable")
            for value in result.stdout[:-1].split(b"\0"):
                path = Path(os.fsdecode(value))
                if not value or not path.is_absolute():
                    raise ValueError("quality_git_path_not_absolute")
                paths.add(path)
    return [_quality_git_file(path) for path in sorted(paths)]


def _quality_git_file(path: Path) -> object:
    try:
        path.lstat()
    except FileNotFoundError:
        return {"path": str(path), "exists": False}
    resolved = path.resolve(strict=True)
    return {"path": str(path), "resolved": str(resolved),
            "file": physical_inventory._file_row(resolved, resolved.parent)}


def _quality_python(environment: Mapping[str, str]) -> str:
    executable = shutil.which("python3", path=environment.get("PATH", ""))
    if executable is None:
        raise ValueError("quality_python_unavailable")
    python = Path(executable).resolve(strict=True)
    return _digest(physical_inventory._file_row(python, python.parent))


def _quality_packages(repo: Path) -> str:
    modules = (repo / "node_modules").resolve(strict=True)
    if modules != repo / "node_modules" or not modules.is_dir():
        raise ValueError("quality_modules_not_canonical")
    packages = [modules / "@swc/core", modules / "@swc/types", modules / "@swc/counter",
                modules / "typescript", modules / "zod"]
    for parent, prefix in ((modules / "@swc", "core-"), (modules / "@typescript", "typescript-")):
        found = sorted(path for path in parent.iterdir() if path.name.startswith(prefix))
        if not found:
            raise ValueError("quality_native_package_missing")
        packages.extend(found)
    return _digest([_checked_inventory(package, modules) for package in packages])


def collect_quality_inputs(repo: Path, environment: Mapping[str, str]) -> str:
    """2件の品質契約に必要な外部入力を固定digestとして取得する。"""
    home = Path(environment.get("HOME", ""))
    if not home.is_absolute() or home.resolve(strict=True) != home:
        raise ValueError("quality_home_not_canonical")
    if any(environment.get(name) for name in OVERRIDES):
        raise ValueError("quality_unsupported_override")
    if any(value and (name.startswith("LD_") or name.startswith("DYLD_"))
           for name, value in environment.items()):
        raise ValueError("quality_unsupported_loader")
    absence = _config_absence(repo, home, environment)
    ruff_absence = _quality_ruff_absence(repo, home, environment)
    git = _quality_git(repo, environment)
    python = _quality_python(environment)
    packages = _quality_packages(repo)
    if (absence != _config_absence(repo, home, environment)
            or ruff_absence != _quality_ruff_absence(repo, home, environment)
            or git != _quality_git(repo, environment)
            or python != _quality_python(environment)
            or packages != _quality_packages(repo)):
        raise ValueError("quality_inputs_changed")
    selected = {name: value for name, value in environment.items()
                if name not in ORCHESTRATION_METADATA or name == "CARGO_CAPACITY_PROTECTED_REFS"}
    return _digest({"git": git, "python": python, "packages": packages, "absence": absence,
                    "ruffAbsence": ruff_absence,
                    "environmentSha256": _digest(selected)})


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
