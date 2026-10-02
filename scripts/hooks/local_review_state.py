from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
from typing import Any
from urllib.parse import urlsplit


class ReviewError(RuntimeError):
    pass


def canonical(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def digest(value: Any) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


def strict_json(raw: str) -> Any:
    def pairs(entries: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in entries:
            if key in result:
                raise ReviewError(f"duplicate JSON key: {key}")
            result[key] = value
        return result

    try:
        return json.loads(raw, object_pairs_hook=pairs)
    except (ValueError, TypeError) as error:
        raise ReviewError("invalid review JSON") from error


def command(arguments: list[str], root: Path, input_bytes: bytes | None = None) -> str:
    environment = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    rtk = shutil.which("rtk")
    if rtk is None:
        if os.environ.get("CI", "").lower() == "true" or os.environ.get("GITHUB_ACTIONS", "").lower() == "true":
            executable = arguments
        else:
            raise ReviewError("rtk is required for local review commands")
    else:
        executable = [rtk, "proxy", *arguments]
    try:
        result = subprocess.run(
            executable, cwd=root, env=environment,
            capture_output=True, text=input_bytes is None, input=input_bytes,
            check=True, timeout=60,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise ReviewError(f"read-only command failed: {arguments[0]}") from error
    return result.stdout.decode() if isinstance(result.stdout, bytes) else result.stdout


def repository_root() -> Path:
    return Path(command(["git", "rev-parse", "--show-toplevel"], Path.cwd()).strip()).resolve()


def file_record(root: Path, name: str) -> dict[str, Any]:
    path = root / name
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return {"kind": "deleted"}
    if stat.S_ISLNK(mode):
        content = os.fsencode(os.readlink(path))
        kind = "symlink"
    elif stat.S_ISREG(mode):
        content = path.read_bytes()
        kind = "file"
    else:
        raise ReviewError(f"unsupported source entry: {name}")
    return {"kind": kind, "executable": bool(mode & 0o111),
            "sha256": hashlib.sha256(content).hexdigest()}


def source_snapshot(root: Path, base: str) -> dict[str, Any]:
    base_sha = command(["git", "rev-parse", "--verify", "--end-of-options", f"{base}^{{commit}}"], root).strip()
    head_sha = command(["git", "rev-parse", "--verify", "HEAD^{commit}"], root).strip()
    names = command(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], root)
    baseline = command(["git", "ls-tree", "-r", "--name-only", "-z", base_sha], root)
    base_names = set(baseline.split("\0")) - {""}
    cached_diff_base_names = set(command(
        ["git", "diff", "--cached", "--no-renames", "--name-only", "-z", base_sha, "--"], root
    ).split("\0")) - {""}
    head_diff_base_names = set(command(
        ["git", "diff", "--no-renames", "--name-only", "-z", base_sha, head_sha, "--"], root
    ).split("\0")) - {""}
    index = command(["git", "ls-files", "--stage", "-z"], root)
    head_tree = command(["git", "ls-tree", "-r", "-z", head_sha], root)
    head_entries = {entry.split("\t", 1)[1]: entry.split("\t", 1)[0]
                    for entry in head_tree.split("\0") if entry}
    entries = [entry for entry in index.split("\0") if entry]
    if any(entry.split("\t", 1)[0].split()[-1] != "0" for entry in entries):
        raise ReviewError("unmerged index cannot be reviewed")
    paths = set(names.split("\0")) | set(baseline.split("\0")) | set(head_entries)
    files = {name: file_record(root, name) for name in sorted(paths - {""})}
    return {"base_sha": base_sha, "files": files,
            "new_tracked_paths": sorted((cached_diff_base_names | head_diff_base_names) - base_names),
            "index_overrides": staged_overrides(root, entries, base_sha, head_sha),
            "head_overrides": head_overrides(root, base_sha, head_sha, head_entries)}


def staged_overrides(root: Path, entries: list[str], base_sha: str, head_sha: str) -> list[str]:
    overrides = []
    staged = command(["git", "diff", "--cached", "--no-renames", "--name-only", "-z", base_sha, "--"], root)
    head = command(["git", "diff", "--no-renames", "--name-only", "-z", base_sha, head_sha, "--"], root)
    candidates = (set(staged.split("\0")) | set(head.split("\0"))) - {""}
    indexed = {entry.split("\t", 1)[1]: entry for entry in entries}
    for name in sorted(candidates):
        entry = indexed.get(name)
        current = working_entry(root, name)
        indexed_value = None if entry is None else entry.split("\t", 1)[0].rsplit(" ", 1)[0]
        if indexed_value != current:
            overrides.append(entry if entry is not None else f"deleted\t{name}")
    return sorted(overrides)


def head_overrides(root: Path, base_sha: str, head_sha: str, entries: dict[str, str]) -> list[str]:
    changed = command(["git", "diff", "--no-renames", "--name-only", "-z", base_sha, head_sha, "--"], root)
    overrides = []
    for name in sorted(set(changed.split("\0")) - {""}):
        entry = entries.get(name)
        fields = None if entry is None else entry.split()
        value = None if fields is None else f"{fields[0]} {fields[2]}"
        if value != working_entry(root, name):
            overrides.append(f"deleted\t{name}" if value is None else f"{value}\t{name}")
    return overrides


def working_entry(root: Path, name: str) -> str | None:
    path = root / name
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return None
    if stat.S_ISLNK(mode):
        content = os.fsencode(os.readlink(path))
        file_mode = "120000"
    elif stat.S_ISREG(mode):
        file_mode = "100755" if mode & 0o111 else "100644"
        blob = command(["git", "hash-object", f"--path={name}", "--", name], root).strip()
        return f"{file_mode} {blob}"
    else:
        raise ReviewError(f"unsupported source entry: {name}")
    blob = command(["git", "hash-object", "--stdin", "--no-filters"], root,
                   input_bytes=content).strip()
    return f"{file_mode} {blob}"


def issue_context(root: Path, numbers: list[int]) -> list[dict[str, Any]]:
    remote = command(["git", "remote", "get-url", "origin"], root).strip()
    prefix = "https://github.com/"
    if remote.startswith("git@github.com:"):
        repository = remote.removeprefix("git@github.com:").removesuffix(".git")
    elif remote.startswith(prefix):
        repository = remote.removeprefix(prefix).removesuffix(".git")
    elif remote.startswith("ssh://"):
        try:
            parsed = urlsplit(remote)
            port = parsed.port
        except ValueError as error:
            raise ReviewError("origin must identify a GitHub repository") from error
        path = parsed.path.removeprefix("/")
        if (parsed.scheme != "ssh" or parsed.hostname != "github.com" or
                parsed.username != "git" or parsed.password is not None or
                port not in (None, 22) or parsed.query or parsed.fragment or
                not path or path.startswith("/") or path.count("/") != 1):
            raise ReviewError("origin must identify a GitHub repository")
        repository = path.removesuffix(".git")
    else:
        raise ReviewError("origin must identify a GitHub repository")
    if repository != "HiroyukiFuruno/katana-render-runtime":
        raise ReviewError("local review is scoped to katana-render-runtime")
    issues = []
    for number in sorted(set(numbers)):
        payload = strict_json(command(["gh", "api", f"repos/{repository}/issues/{number}"], root))
        if not isinstance(payload, dict) or payload.get("number") != number or "pull_request" in payload:
            raise ReviewError("Issue response does not identify the requested Issue")
        fields = ("title", "body", "state", "html_url")
        if any(not isinstance(payload.get(key), str) or not payload[key] for key in fields):
            raise ReviewError("Issue response is incomplete")
        if payload["state"] != "open":
            raise ReviewError(f"Issue #{number} must remain open during local review")
        if payload["html_url"] != f"https://github.com/{repository}/issues/{number}":
            raise ReviewError("Issue URL does not match its repository identity")
        issue = {key: payload[key] for key in fields}
        issue["number"] = number
        issue["body_sha256"] = hashlib.sha256(payload["body"].encode()).hexdigest()
        issues.append(issue)
    return issues


def requirements_context(root: Path, path: str | None) -> dict[str, str] | None:
    if path is None:
        return None
    source = (root / path).resolve()
    if not source.is_relative_to(root) or not source.is_file():
        raise ReviewError("requirements must be a repository file")
    content = source.read_text()
    if not content.strip():
        raise ReviewError("requirements file is empty")
    return {"path": str(source.relative_to(root)), "content": content,
            "sha256": hashlib.sha256(content.encode()).hexdigest()}


def gate_configuration(root: Path) -> dict[str, str]:
    names = ("COVERAGE_MIN_LINES", "COVERAGE_MAX_UNCOVERED_LINES", "TEST_THREADS",
             "RUSTFLAGS", "CARGO", "JOBS", "CHECK_JOBS")
    values = {name: command(["just", "--justfile", str(root / "Justfile"),
                            "--evaluate", name], root).rstrip("\n") for name in names}
    cargo_names = ("CARGO_BUILD_TARGET", "CARGO_BUILD_RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS",
                   "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN")
    values.update({name: os.environ.get(name, "") for name in cargo_names})
    values["RUSTC"] = os.environ.get("RUSTC", "rustc")
    target = os.environ.get("CARGO_TARGET_DIR", "target")
    values["CARGO_TARGET_DIR"] = str((root / target).resolve())
    build = os.environ.get("CARGO_BUILD_BUILD_DIR", target)
    values["CARGO_BUILD_BUILD_DIR"] = str((root / build).resolve())
    coverage_target = os.environ.get("CARGO_LLVM_COV_TARGET_DIR", str(Path(target) / "llvm-cov-target"))
    coverage_build = os.environ.get("CARGO_LLVM_COV_BUILD_DIR", coverage_target) if "CARGO_LLVM_COV_TARGET_DIR" in os.environ else str(Path(build) / "llvm-cov-target")
    values["CARGO_LLVM_COV_TARGET_DIR"] = str((root / coverage_target).resolve())
    values["CARGO_LLVM_COV_BUILD_DIR"] = str((root / coverage_build).resolve())
    return values


def cache_path(root: Path, value: str) -> Path:
    path = (root / value).resolve()
    cache_root = (root / "tmp").resolve()
    if not cache_root.is_relative_to(root) or not path.is_relative_to(cache_root):
        raise ReviewError("review evidence must be stored under repository tmp/")
    if path == cache_root:
        raise ReviewError("receipt path must identify a file")
    return path
