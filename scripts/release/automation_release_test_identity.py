"""Build a private identity for the fixed Python release-test bootstrap."""

from __future__ import annotations

import hashlib
import json
import os
import platform
import re
import shutil
import stat
import subprocess
import sys
from collections.abc import Mapping
from pathlib import Path

from native_coverage_local_context import UNSUPPORTED_ENV as NATIVE_UNSUPPORTED_ENV
from native_coverage_local_context import VOLATILE_ENV as NATIVE_VOLATILE_ENV
from html_performance_inventory import _stamp, inventory
from runtime_script_tool_fingerprint_inputs import OVERRIDES, _config_absence, _quality_git

SHA256 = re.compile(r"[0-9a-f]{64}")
RELEASE_TEST_PROGRAM = "scripts/release/automation_release_test_cases.py"
DERIVED_LAUNCH_ENVIRONMENT = "KRR_HTML_PERFORMANCE_ENVIRONMENT_SHA256"
EXTERNAL_TOOLS = ("bash", "bun", "cargo", "git", "just", "node", "python3", "rtk", "rustc", "rustup", "uv")
EXTERNAL_GIT_CONTEXT = frozenset({
    "GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_CEILING_DIRECTORIES", "GIT_NAMESPACE", "GIT_SHALLOW_FILE",
})


def _digest(value: object) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()
    return hashlib.sha256(encoded).hexdigest()


def _git(repo: Path, environment: dict[str, str], *arguments: str) -> bytes:
    result = subprocess.run(
        ["git", *arguments], cwd=repo, env=environment, capture_output=True, check=True,
    )
    return result.stdout


def _source_state(repo: Path, environment: dict[str, str]) -> str | None:
    """Digest all tracked and non-ignored untracked regular files in the checkout."""
    try:
        output = _git(repo, environment, "ls-files", "--cached", "--others", "--exclude-standard", "-z")
        refs = _git(repo, environment, "show-ref", "--head")
    except (OSError, subprocess.SubprocessError):
        return None
    if not output.endswith(b"\0") or not refs:
        return None
    names = sorted(set(output[:-1].split(b"\0")))
    records: list[dict[str, object]] = []
    for raw_name in names:
        try:
            name = os.fsdecode(raw_name)
            path = repo / name
            before = _stamp(path)
            if path.resolve(strict=True) != path or not stat.S_ISREG(before[2]):
                return None
            with path.open("rb") as source:
                digest = hashlib.file_digest(source, "sha256").hexdigest()
            if before != _stamp(path):
                return None
        except (OSError, ValueError):
            return None
        records.append({"path": name, "mode": before[2], "size": before[3], "sha256": digest})
    return _digest({"files": records, "refsSha256": hashlib.sha256(refs).hexdigest()})


def _executable_sha256(name: str, environment: Mapping[str, str]) -> str | None:
    executable = shutil.which(name, path=environment.get("PATH", ""))
    if executable is None:
        return None
    try:
        path = Path(executable).resolve(strict=True)
        if not path.is_file():
            return None
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError:
        return None


def _stable_environment(environment: Mapping[str, str]) -> dict[str, str] | None:
    values: dict[str, str] = {}
    for name, value in environment.items():
        # 元の環境を下で照合する。生の環境ハッシュを含めると、除外した実行メタデータが再びキーに入る。
        if name == DERIVED_LAUNCH_ENVIRONMENT or name in NATIVE_VOLATILE_ENV:
            continue
        if value and (name in NATIVE_UNSUPPORTED_ENV or name in OVERRIDES or name in EXTERNAL_GIT_CONTEXT
                      or name.startswith(("LD_", "DYLD_"))
                      or name in {"PYTHONPATH", "PYTHONHOME", "PYTHONPYCACHEPREFIX"}):
            return None
        values[name] = value
    return values


def input_digest(
    repo: Path,
    environment: dict[str, str],
    installed_identity: str,
) -> str | None:
    """Return a sealed identity, or ``None`` when any required input is unknown."""
    if not repo.is_dir() or not isinstance(installed_identity, str) or SHA256.fullmatch(installed_identity) is None:
        return None
    stable_environment = _stable_environment(environment)
    if stable_environment is None:
        return None
    executable_sha256 = {
        name: _executable_sha256(name, stable_environment)
        for name in (*EXTERNAL_TOOLS, sys.executable)
    }
    if any(value is None for value in executable_sha256.values()):
        return None
    source = _source_state(repo, stable_environment)
    if source is None:
        return None
    try:
        git_configuration = _quality_git(repo, stable_environment)
        absence = _config_absence(repo, Path(stable_environment.get("HOME", "")), stable_environment)
        modules = repo / "node_modules"
        if not modules.is_dir() or modules.resolve(strict=True) != modules:
            return None
        node_modules = inventory(modules)
        for path in modules.rglob("*"):
            if path.is_symlink() and not path.resolve().is_relative_to(modules):
                return None
    except (OSError, ValueError):
        return None
    command = {"program": RELEASE_TEST_PROGRAM, "cwd": "repository-root", "scope": "scripts/release"}
    return _digest(
        {
            "schema": "krr-automation-release-test-input-v1",
            "sourceSha256": source,
            "command": command,
            "executableSha256": executable_sha256,
            "gitConfigurationSha256": git_configuration,
            "externalConfigurationAbsent": absence,
            "environmentSha256": _digest(stable_environment),
            "nodeModulesSha256": _digest(node_modules),
            "installedIdentitySha256": installed_identity,
            "platform": platform.platform(),
        }
    )
