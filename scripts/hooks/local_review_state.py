from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
from typing import Any
from urllib.parse import unquote_to_bytes, urlsplit


class ReviewError(RuntimeError):
    pass


LOCAL_REPOSITORY = "HiroyukiFuruno/katana-render-runtime"


def canonical(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":")).encode("utf-8")


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
            capture_output=True,
            text=input_bytes is None and not (arguments[0] == "git" and "-z" in arguments),
            input=input_bytes,
            check=True, timeout=60,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise ReviewError(f"read-only command failed: {arguments[0]}") from error
    return os.fsdecode(result.stdout) if isinstance(result.stdout, bytes) else result.stdout


def repository_root() -> Path:
    return Path(command(["git", "rev-parse", "--show-toplevel"], Path.cwd()).strip()).resolve()


def file_record(root: Path, name: str) -> dict[str, Any]:
    path = root / name
    try:
        mode = path.lstat().st_mode
    except (FileNotFoundError, NotADirectoryError):
        return {"kind": "deleted"}
    if stat.S_ISLNK(mode):
        content = os.fsencode(os.readlink(path))
        kind = "symlink"
    elif stat.S_ISREG(mode):
        content = path.read_bytes()
        kind = "file"
    elif stat.S_ISDIR(mode):
        return {"kind": "deleted"}
    else:
        raise ReviewError(f"unsupported source entry: {name}")
    return {"kind": kind, "executable": bool(mode & stat.S_IXUSR),
            "sha256": hashlib.sha256(content).hexdigest()}


def ensure_normal_index_flags(root: Path) -> None:
    for option in ("-v", "-f"):
        records = command(["git", "ls-files", option, "-z"], root).split("\0")
        for record in records:
            if record and not record.startswith("H "):
                marker = record[0] if record else "?"
                raise ReviewError(f"git index has unsupported {option} marker: {marker!r}")


def source_snapshot(root: Path, base: str) -> dict[str, Any]:
    base_sha = command(["git", "rev-parse", "--verify", "--end-of-options", f"{base}^{{commit}}"], root).strip()
    head_sha = command(["git", "rev-parse", "--verify", "HEAD^{commit}"], root).strip()
    ensure_normal_index_flags(root)
    names = command(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], root)
    baseline = command(["git", "ls-tree", "-r", "--name-only", "-z", base_sha], root)
    base_names = set(baseline.split("\0")) - {""}
    cached_diff_base_names = set(command(
        ["git", "diff", "--cached", "--no-renames", "--name-only", "-z", base_sha, "--"], root
    ).split("\0")) - {""}
    head_diff_base_names = set(command(
        ["git", "diff", "--no-renames", "--name-only", "-z", base_sha, head_sha, "--"], root
    ).split("\0")) - {""}
    working_diff_base_names = set(command(
        ["git", "diff", "--no-renames", "--name-only", "-z", base_sha, "--"], root
    ).split("\0")) - {""}
    untracked_names = set(command(
        ["git", "ls-files", "--others", "--exclude-standard", "-z"], root
    ).split("\0")) - {""}
    index = command(["git", "ls-files", "--stage", "-z"], root)
    head_tree = command(["git", "ls-tree", "-r", "-z", head_sha], root)
    head_entries = {entry.split("\t", 1)[1]: entry.split("\t", 1)[0]
                    for entry in head_tree.split("\0") if entry}
    entries = [entry for entry in index.split("\0") if entry]
    if any(entry.split("\t", 1)[0].split()[-1] != "0" for entry in entries):
        raise ReviewError("unmerged index cannot be reviewed")
    submodule_paths = {
        entry.split("\t", 1)[1]
        for entry in entries
        if entry.split("\t", 1)[0].split()[0] == "160000"
    }
    submodule_paths.update(
        name for name, entry in head_entries.items() if entry.split()[0] == "160000"
    )
    if submodule_paths:
        raise ReviewError(f"submodule entries cannot be reviewed: {', '.join(sorted(submodule_paths))}")
    paths = set(names.split("\0")) | set(baseline.split("\0")) | set(head_entries)
    files = {name: file_record(root, name) for name in sorted(paths - {""})}
    working_paths = working_diff_base_names | cached_diff_base_names | head_diff_base_names | untracked_names
    working_blobs = {name: working_entry(root, name) for name in sorted(working_paths)}
    candidate_paths = cached_diff_base_names | head_diff_base_names
    omitted_working_paths = (sorted(working_diff_base_names - candidate_paths)
                             if candidate_paths or head_sha != base_sha else [])
    return {"base_sha": base_sha, "files": files,
            "new_tracked_paths": sorted((cached_diff_base_names | head_diff_base_names) - base_names),
            "index_overrides": staged_overrides(root, entries, base_sha, head_sha),
            "head_overrides": head_overrides(root, base_sha, head_sha, head_entries),
            "working_blobs": working_blobs,
            "omitted_working_paths": omitted_working_paths}


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
    except (FileNotFoundError, NotADirectoryError):
        return None
    if stat.S_ISLNK(mode):
        content = os.fsencode(os.readlink(path))
        file_mode = "120000"
    elif stat.S_ISREG(mode):
        file_mode = "100755" if mode & stat.S_IXUSR else "100644"
        blob = command(["git", "hash-object", f"--path={name}", "--", name], root).strip()
        return f"{file_mode} {blob}"
    elif stat.S_ISDIR(mode):
        return None
    else:
        raise ReviewError(f"unsupported source entry: {name}")
    blob = command(["git", "hash-object", "--stdin", "--no-filters"], root,
                   input_bytes=content).strip()
    return f"{file_mode} {blob}"


def normalize_remote_url(remote: str) -> str:
    hex_digits = frozenset("0123456789abcdefABCDEF")
    for index, character in enumerate(remote):
        if (character == "%" and
                (index + 2 >= len(remote) or
                 remote[index + 1] not in hex_digits or remote[index + 2] not in hex_digits)):
            raise ReviewError("origin must identify a GitHub repository")
    try:
        raw = urlsplit(remote)
        normalized = unquote_to_bytes(remote).decode("utf-8")
        decoded = urlsplit(normalized)

        def decode_component(value: str | None) -> str | None:
            return None if value is None else unquote_to_bytes(value).decode("utf-8")

        raw_hostname = decode_component(raw.hostname)
        if (raw_hostname is not None and raw_hostname.lower() != decoded.hostname) or \
                raw.scheme != decoded.scheme or \
                decode_component(raw.username) != decoded.username or \
                decode_component(raw.password) != decoded.password or \
                decode_component(raw.path) != decoded.path:
            raise ReviewError("origin must identify a GitHub repository")
    except (UnicodeError, ValueError) as error:
        raise ReviewError("origin must identify a GitHub repository") from error
    if (any(ord(character) <= 32 or ord(character) == 127 or character.isspace()
            for character in normalized) or "?" in normalized or "#" in normalized):
        raise ReviewError("origin must identify a GitHub repository")
    return normalized


def issue_context(root: Path, numbers: list[int]) -> list[dict[str, Any]]:
    remote = command(["git", "remote", "get-url", "origin"], root, input_bytes=b"").removesuffix("\n")
    if any(ord(character) <= 32 or ord(character) == 127 for character in remote):
        raise ReviewError("origin must identify a GitHub repository")
    elif "@" in remote and ":" in remote and not remote.startswith(("https://", "ssh://")):
        username, separator, authority_path = remote.partition("@")
        host, separator, path = authority_path.partition(":")
        if (separator != ":" or username != "git" or host.casefold() != "github.com" or
                "@" in host or not path):
            raise ReviewError("origin must identify a GitHub repository")
        repository = path.removesuffix(".git")
    elif remote.startswith(("https://", "ssh://")):
        remote = normalize_remote_url(remote)
        try:
            parsed = urlsplit(remote)
            port = parsed.port
        except ValueError as error:
            raise ReviewError("origin must identify a GitHub repository") from error
        authority = parsed.netloc.rsplit("@", 1)[-1]
        if authority.endswith(":"):
            raise ReviewError("origin must identify a GitHub repository")
        path = parsed.path.removeprefix("/")
        if parsed.scheme == "https":
            valid = (parsed.hostname == "github.com" and parsed.username is None and
                     parsed.password is None and port in (None, 443))
        else:
            valid = (parsed.hostname == "github.com" and parsed.username == "git" and
                     parsed.password is None and port in (None, 22))
        if (not valid or parsed.query or parsed.fragment or
                not path or path.startswith("/") or path.count("/") != 1):
            raise ReviewError("origin must identify a GitHub repository")
        repository = path.removesuffix(".git")
    else:
        raise ReviewError("origin must identify a GitHub repository")
    if repository.casefold() != LOCAL_REPOSITORY.casefold():
        raise ReviewError("local review is scoped to katana-render-runtime")
    # GitHubが大小文字を区別しないため、APIとURL照合には同じ正規名を使う。
    repository = LOCAL_REPOSITORY
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
                   "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER",
                   "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET_DIR",
                   "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN")
    values.update({name: os.environ.get(name, "") for name in cargo_names})
    values["RUSTC"] = os.environ.get("RUSTC", "rustc")
    values.update({f"{name}_PRESENT": str(name in os.environ).lower()
                   for name in (*cargo_names, "RUSTC")})
    for name in ("CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR"):
        if name in os.environ and not os.environ[name]:
            raise ReviewError(f"{name} must not be empty")
    if "CARGO_BUILD_TARGET_DIR" in os.environ:
        values["CARGO_BUILD_TARGET_DIR"] = str((root / os.environ["CARGO_BUILD_TARGET_DIR"]).resolve())
    target = os.environ.get("CARGO_TARGET_DIR", values["CARGO_BUILD_TARGET_DIR"] or "target")
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
