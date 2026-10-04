#!/usr/bin/env python3
"""Collect and optionally publish owner-asserted local macOS CI evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import stat
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable
from urllib import error, request


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/test-and-build.yml"
SUPPORTED_WORKFLOW_SHA256 = "8e62fe210cd344e7532d36d1cc2c505873f9a22ed6b6ff40e9c4b3d22c401358"
SCHEMA = 1
FRESHNESS_SECONDS = 24 * 60 * 60
MARKER = "<!-- krr-local-macos-evidence:v1 -->"
SHA_RE = re.compile(r"^[0-9a-f]{40}$")

# 品質対象は test-and-build.yml の mac-arm64 実行内容と一致させる。
COMMANDS: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("bun-install", ("bun", "install", "--frozen-lockfile")),
    ("graphviz-install", ("brew", "install", "graphviz")),
    ("plantuml-install", ("just", "plantuml-install")),
    ("rebuild-v8", ("cargo", "clean", "-p", "v8")),
    ("build-cli", ("cargo", "build", "-p", "katana-render-runtime-cli", "--locked")),
    ("rust-check", ("cargo", "check", "--workspace", "--locked")),
    ("format", ("just", "fmt-check")),
    ("lint", ("just", "lint")),
    ("ast-lint", ("just", "ast-lint")),
    ("runtime-bundle", ("just", "runtime-bundle-check")),
    ("unit-tests", ("just", "unit-test")),
    (
        "plantuml-render",
        (
            "cargo", "run", "-p", "katana-render-runtime-cli", "--", "plantuml", "render",
            "--input", "tests/fixtures/plantuml/representative/01-sequence.puml",
            "--output", "{plantuml_output}",
        ),
    ),
    ("dependency-boundary", ("just", "dependency-leak")),
    ("typescript-lint", ("just", "biome")),
    ("typescript-types", ("just", "typecheck")),
    ("runtime-asset-checksums", ("just", "runtime-asset-check")),
)
SCOPE_PARAMETERS = {
    "CARGO": "cargo",
    "CARGO_BUILD_TARGET": "aarch64-apple-darwin",
    "CARGO_BUILD_RUSTFLAGS": "-D warnings",
    "CARGO_BUILD_RUSTC": "rustc",
    "CARGO_BUILD_RUSTC_WRAPPER": "",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER": "",
    "CARGO_ENCODED_RUSTFLAGS": "-D\x1fwarnings",
    "CARGO_INCREMENTAL": "0",
    "CARGO_TERM_COLOR": "always",
    "CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER": "/usr/bin/env",
    "RTK": "",
    "RUSTFLAGS": "-D warnings",
    "RUSTC": "rustc",
    "RUSTC_WRAPPER": "",
    "RUSTC_WORKSPACE_WRAPPER": "",
    "TEST_THREADS": "1",
}
PREPARATION_COMMANDS = {"bun-install", "graphviz-install", "plantuml-install"}
RUST_STABLE_MANIFEST_URL = "https://static.rust-lang.org/dist/channel-rust-stable.toml"
MAX_RUST_STABLE_MANIFEST_BYTES = 8 * 1024 * 1024

# macOS対象stepを列挙し、workflow全体の固定hashと合わせて未知の変更を拒否する。
MAC_WORKFLOW_STEP_COMMANDS: dict[str, tuple[str, ...]] = {
    "Verify optional local macOS evidence": (),
    "Report reused local macOS evidence": (),
    "Install Java runtime": (),
    "Install Rust toolchain": (),
    "Cache Rust build outputs": (),
    "Install Bun": (),
    "Install JavaScript dependencies": ("bun-install",),
    "Install just": (),
    "Rebuild V8 static archive on macOS arm64": ("rebuild-v8", "build-cli"),
    "Check Rust types": ("rust-check",),
    "Check formatting": ("format",),
    "Run linter": ("lint",),
    "Run AST linter": ("ast-lint",),
    "Check runtime bundle sync": ("runtime-bundle",),
    "Install Graphviz for PlantUML on macOS": ("graphviz-install",),
    "Install PlantUML runtime": ("plantuml-install",),
    "Run tests": ("unit-tests",),
    "Run PlantUML smoke check": ("plantuml-render",),
    "Check dependency boundary": ("dependency-boundary",),
    "Check TypeScript formatting and lint": ("typescript-lint",),
    "Check TypeScript types": ("typescript-types",),
    "Check runtime asset checksums": ("runtime-asset-checksums",),
}
class EvidenceError(Exception):
    pass


def canonical(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def scope_digest(workflow_sha: str | None = None) -> str:
    body = {
        "schema": SCHEMA, "commands": COMMANDS, "environment": SCOPE_PARAMETERS,
        "workflow_sha256": workflow_sha or workflow_digest(),
    }
    return sha256_bytes(canonical(body))


def read_comments_api(token: str, repository: str, number: int) -> list[Any]:
    comments: list[Any] = []
    page = 1
    while True:
        values = api_request(
            token, "GET", f"repos/{repository}/issues/{number}/comments?per_page=100&page={page}"
        )
        if not isinstance(values, list) or len(values) > 100:
            raise EvidenceError("PR comments response is invalid")
        comments.extend(values)
        if len(values) < 100:
            return comments
        page += 1


def workflow_digest(path: Path = WORKFLOW) -> str:
    return sha256_bytes(path.read_bytes())


def workflow_scope_supported(path: Path = WORKFLOW) -> bool:
    """workflowが明示承認済みの内容と一致する場合だけlocal証跡を許可する。"""
    try:
        return sha256_bytes(path.read_bytes()) == SUPPORTED_WORKFLOW_SHA256
    except OSError:
        return False


def run_git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args], cwd=ROOT, check=True, text=True, capture_output=True
    )
    return result.stdout.strip()


def git_status() -> str:
    return run_git("status", "--porcelain", "--untracked-files=all")


def require_standard_index_flags() -> None:
    # WHY: Git status は assume-unchanged と skip-worktree の変更を隠す。
    entries = run_git("ls-files", "-v", "-z").split("\0")
    if any(entry and entry[0] != "H" for entry in entries):
        raise EvidenceError("nonstandard Git index flags invalidate local proof")


def head_worktree_bytes_digest(expected_head: str) -> str:
    object_format = run_git("rev-parse", "--show-object-format")
    if object_format not in {"sha1", "sha256"}:
        raise EvidenceError("unsupported Git object format")
    try:
        tree = subprocess.run(
            ["git", "ls-tree", "-r", "-z", "--full-tree", expected_head],
            cwd=ROOT, check=True, capture_output=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        raise EvidenceError("cannot read tracked HEAD tree") from exc
    records: list[bytes] = []
    for entry in tree.split(b"\0"):
        if not entry:
            continue
        try:
            metadata, name = entry.split(b"\t", 1)
            mode_bytes, kind, object_id = metadata.split(b" ", 2)
            mode = mode_bytes.decode("ascii")
        except (ValueError, UnicodeDecodeError) as exc:
            raise EvidenceError("HEAD tree entry is malformed") from exc
        if mode == "160000" or kind != b"blob" or mode not in {"100644", "100755", "120000"}:
            raise EvidenceError("tracked gitlinks or unsupported tracked file types invalidate local proof")
        path = ROOT / os.fsdecode(name)
        try:
            file_mode = path.lstat().st_mode
            if stat.S_ISLNK(file_mode):
                working_mode = "120000"
                working_bytes = os.fsencode(os.readlink(path))
            elif stat.S_ISREG(file_mode):
                working_mode = "100755" if file_mode & stat.S_IXUSR else "100644"
                working_bytes = path.read_bytes()
            else:
                raise EvidenceError(f"unsupported tracked worktree entry: {os.fsdecode(name)}")
        except OSError as exc:
            raise EvidenceError(f"tracked worktree entry is unavailable: {os.fsdecode(name)}") from exc
        digest = hashlib.new(object_format)
        digest.update(f"blob {len(working_bytes)}\0".encode("ascii"))
        digest.update(working_bytes)
        if working_mode != mode or digest.hexdigest().encode("ascii") != object_id:
            raise EvidenceError(f"tracked worktree bytes differ from HEAD: {os.fsdecode(name)}")
        records.append(mode_bytes + b" " + object_id + b"\t" + name + b"\0")
    if run_git("rev-parse", "HEAD") != expected_head:
        raise EvidenceError("HEAD changed while checking tracked worktree bytes")
    return sha256_bytes(b"".join(records))


def macos_product_version() -> str:
    result = subprocess.run(
        ["sw_vers", "-productVersion"], cwd=ROOT, check=True, text=True, capture_output=True,
    )
    version = result.stdout.strip()
    if not version:
        raise EvidenceError("macOS product version is unavailable")
    return version


def is_macos_15(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"15\.\d+(?:\.\d+)?", value.strip()) is not None


def command_environment() -> dict[str, str]:
    environment = os.environ.copy()
    unsupported = {
        name for name in environment
        if name in {"RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"}
        or name.startswith("CARGO_BUILD_RUSTC")
        or (name.startswith("CARGO_TARGET_") and name != "CARGO_TARGET_DIR")
    }
    if unsupported:
        raise EvidenceError("custom Rust compiler or target linker environment is unsupported")
    environment.update(SCOPE_PARAMETERS)
    return environment


def gh_token() -> str:
    result = subprocess.run(["gh", "auth", "token"], check=True, text=True, capture_output=True)
    token = result.stdout.strip()
    if not token:
        raise EvidenceError("gh auth token returned an empty token")
    return token


def api_request(token: str, method: str, path: str, body: dict[str, Any] | None = None) -> Any:
    headers = {
        "Accept": "application/vnd.github+json",
        "Authorization": f"Bearer {token}",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "katana-render-runtime-local-macos-evidence",
    }
    data = canonical(body) if body is not None else None
    if data is not None:
        headers["Content-Type"] = "application/json"
    req = request.Request(f"https://api.github.com/{path}", data=data, headers=headers, method=method)
    try:
        with request.urlopen(req, timeout=20) as response:
            return json.loads(response.read().decode("utf-8"))
    except (error.URLError, error.HTTPError, TimeoutError, OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise EvidenceError("GitHub API request failed") from exc


def canonical_owner(repo_payload: Any, repository: str) -> str:
    if not isinstance(repo_payload, dict):
        raise EvidenceError("repository API response is invalid")
    owner = repo_payload.get("owner")
    if not isinstance(owner, dict) or not isinstance(owner.get("login"), str) or not owner["login"].strip():
        raise EvidenceError("repository owner is invalid")
    if repo_payload.get("full_name", "").lower() != repository.lower():
        raise EvidenceError("repository identity changed")
    return owner["login"]


def base_is_merged(compare_payload: Any, base_sha: str) -> bool:
    if not isinstance(compare_payload, dict):
        return False
    merge_base = compare_payload.get("merge_base_commit")
    return isinstance(merge_base, dict) and merge_base.get("sha") == base_sha


def validate_pr(value: Any, repository: str, number: int) -> tuple[str, str]:
    if not isinstance(value, dict) or value.get("number") != number or value.get("state") != "open":
        raise EvidenceError("pull request identity or state is invalid")
    base, head = value.get("base"), value.get("head")
    if not isinstance(base, dict) or not isinstance(head, dict):
        raise EvidenceError("pull request SHAs are missing")
    base_sha, head_sha = base.get("sha"), head.get("sha")
    base_repo, head_repo = base.get("repo"), head.get("repo")
    if (
        not isinstance(base_sha, str) or SHA_RE.fullmatch(base_sha) is None
        or not isinstance(head_sha, str) or SHA_RE.fullmatch(head_sha) is None
        or not isinstance(base_repo, dict) or base_repo.get("full_name", "").lower() != repository.lower()
        or not isinstance(head_repo, dict) or head_repo.get("full_name", "").lower() != repository.lower()
        or base.get("ref") != "master"
    ):
        raise EvidenceError("pull request repository or SHA does not match")
    return base_sha, head_sha


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def iso_timestamp(value: Any) -> int:
    if not isinstance(value, str):
        raise EvidenceError("timestamp is missing")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise EvidenceError("timestamp is invalid") from exc
    if parsed.tzinfo is None:
        raise EvidenceError("timestamp has no timezone")
    return int(parsed.timestamp())


def tool_versions() -> dict[str, str]:
    commands = {
        "macos": ("sw_vers", "-productVersion"),
        "architecture": ("uname", "-m"),
        "rustc": ("rustc", "--version"),
        "cargo": ("cargo", "--version"),
        "java": ("java", "-version"),
        "bun": ("bun", "--version"),
        "just": ("just", "--version"),
        "brew": ("brew", "--version"),
        "graphviz": ("dot", "-V"),
    }
    found: dict[str, str] = {}
    for name, command in commands.items():
        result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=True)
        value = (result.stdout + result.stderr).strip().splitlines()
        if not value or not value[0].strip():
            raise EvidenceError(f"tool version is empty: {name}")
        found[name] = value[0].strip()
    if found["architecture"] != "arm64":
        raise EvidenceError("local proof requires Apple Silicon (arm64)")
    if platform.system() != "Darwin":
        raise EvidenceError("local proof requires macOS")
    if not is_macos_15(found["macos"]):
        raise EvidenceError("local proof requires macOS 15, matching the hosted macOS runner")
    rust_verbose = subprocess.run(
        ("rustc", "-vV"), cwd=ROOT, text=True, capture_output=True, check=True,
    ).stdout
    host_target = re.search(r"(?m)^host: (\S+)$", rust_verbose)
    if host_target is None or host_target.group(1) != SCOPE_PARAMETERS["CARGO_BUILD_TARGET"]:
        raise EvidenceError("local Rust host target is not native Apple Silicon macOS")
    found["rust_host"] = host_target.group(1)
    validate_pinned_tools(found)
    return found


def java_is_21(value: Any) -> bool:
    return isinstance(value, str) and re.search(
        r'\bversion\s+["\']?21(?:[.+"\'\s]|$)', value, re.IGNORECASE
    ) is not None


def validate_pinned_tools(versions: Any) -> None:
    if not isinstance(versions, dict) or not java_is_21(versions.get("java")):
        raise EvidenceError("local proof requires Java 21, matching the macOS CI setup")
    if versions.get("bun") != "1.4.2":
        raise EvidenceError("local proof requires Bun 1.4.2, matching the macOS CI setup")


def parse_stable_manifest_versions(manifest: str) -> dict[str, str]:
    """Derive stable CLI versions from the official channel manifest."""
    result: dict[str, str] = {}
    for package in ("rustc", "cargo"):
        section = re.search(
            rf"(?ms)^\[pkg\.{package}\]\s*$\n(?P<body>.*?)(?=^\[|\Z)", manifest
        )
        version = re.search(r'^version\s*=\s*"([^"]+)"\s*$', section["body"], re.MULTILINE) if section else None
        if version is None:
            raise EvidenceError(f"official Rust stable manifest is missing {package} version")
        result[package] = version.group(1)
    # Cargo内部版は0.xのため、Rustと同期するCLI版とCargo自身のrevisionを組み合わせる。
    rust_release = re.match(r"^(\d+\.\d+\.\d+)(?: \([^)]*\))?$", result["rustc"])
    cargo_commit = re.fullmatch(r"0\.\d+\.\d+ \(([0-9a-f]{9,40}) (\d{4}-\d{2}-\d{2})\)", result["cargo"])
    if rust_release is None or cargo_commit is None:
        raise EvidenceError("official Rust stable manifest has unsupported tool version metadata")
    result["cargo_cli"] = f"{rust_release.group(1)} ({cargo_commit.group(1)[:9]} {cargo_commit.group(2)})"
    return result


def fetch_stable_manifest_versions() -> dict[str, str]:
    req = request.Request(
        RUST_STABLE_MANIFEST_URL,
        headers={"Accept": "text/plain", "User-Agent": "katana-render-runtime-local-macos-evidence"},
    )
    try:
        with request.urlopen(req, timeout=15) as response:
            body = response.read(MAX_RUST_STABLE_MANIFEST_BYTES + 1)
        if not body or len(body) > MAX_RUST_STABLE_MANIFEST_BYTES:
            raise EvidenceError("official Rust stable manifest is empty or too large")
        return parse_stable_manifest_versions(body.decode("utf-8"))
    except (error.URLError, error.HTTPError, TimeoutError, OSError, UnicodeDecodeError) as exc:
        raise EvidenceError("official Rust stable manifest is unavailable") from exc


def validate_stable_rust_tools(tools: Any, stable_versions: Any) -> None:
    if not isinstance(tools, dict) or not isinstance(stable_versions, dict):
        raise EvidenceError("Rust toolchain evidence is invalid")
    for package, version_key in (("rustc", "rustc"), ("cargo", "cargo_cli")):
        expected = stable_versions.get(version_key)
        actual = tools.get(package)
        if not isinstance(expected, str) or not isinstance(actual, str) or actual != f"{package} {expected}":
            raise EvidenceError(f"local {package} does not match the current official stable channel")


def parse_comment(comment: Any) -> dict[str, Any] | None:
    if not isinstance(comment, dict) or not isinstance(comment.get("body"), str):
        return None
    body = comment["body"]
    if not body.startswith(MARKER + "\n"):
        return None
    try:
        payload = json.loads(body[len(MARKER) + 1 :], object_pairs_hook=reject_duplicate_keys)
    except (json.JSONDecodeError, ValueError):
        return None
    return payload if isinstance(payload, dict) else None


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key")
        result[key] = value
    return result


def valid_payload(
    comment: dict[str, Any], payload: dict[str, Any], repository: str, number: int,
    base_sha: str, head_sha: str, expected_workflow_digest: str, now: int, owner_login: str,
    stable_versions: dict[str, str],
) -> bool:
    user, association = comment.get("user"), comment.get("author_association")
    if not isinstance(user, dict) or user.get("login", "").lower() != owner_login.lower() or association != "OWNER":
        return False
    required = {
        "schema", "repository", "pull_request", "base_sha", "head_sha", "workflow_sha256",
        "scope_sha256", "completed_at", "platform", "tools", "environment", "commands", "logs_sha256",
    }
    if set(payload) != required:
        return False
    if (
        type(payload.get("schema")) is not int or payload.get("schema") != SCHEMA
        or payload.get("repository", "").lower() != repository.lower()
        or type(payload.get("pull_request")) is not int or payload.get("pull_request") != number
        or payload.get("base_sha") != base_sha
        or payload.get("head_sha") != head_sha
        or payload.get("workflow_sha256") != expected_workflow_digest
        or payload.get("scope_sha256") != scope_digest(expected_workflow_digest)
        or payload.get("platform") != "macos-arm64"
    ):
        return False
    completed = iso_timestamp(payload.get("completed_at"))
    created = iso_timestamp(comment.get("created_at"))
    updated = iso_timestamp(comment.get("updated_at"))
    if created != updated or abs(completed - created) > 300 or now - completed < 0 or now - completed > FRESHNESS_SECONDS:
        return False
    tools = payload.get("tools")
    if not isinstance(tools, dict) or set(tools) != {
        "macos", "architecture", "rust_host", "rustc", "cargo", "java", "bun", "just", "brew", "graphviz"
    } or any(not isinstance(value, str) or not value.strip() for value in tools.values()):
        return False
    if (tools.get("architecture") != "arm64" or not is_macos_15(tools.get("macos"))
            or tools.get("rust_host") != SCOPE_PARAMETERS["CARGO_BUILD_TARGET"]):
        return False
    if not java_is_21(tools.get("java")) or tools.get("bun") != "1.4.2":
        return False
    try:
        validate_stable_rust_tools(tools, stable_versions)
    except EvidenceError:
        return False
    if payload.get("environment") != SCOPE_PARAMETERS:
        return False
    commands = payload.get("commands")
    if not isinstance(commands, list) or len(commands) != len(COMMANDS):
        return False
    for actual, (command_id, argv) in zip(commands, COMMANDS, strict=True):
        if (
            not isinstance(actual, dict) or set(actual) != {"id", "argv", "exit_code", "duration_ms", "log_sha256"}
            or actual.get("id") != command_id or actual.get("argv") != list(argv)
            or type(actual.get("exit_code")) is not int or actual.get("exit_code") != 0
            or type(actual.get("duration_ms")) is not int
            or actual["duration_ms"] < 0 or not is_sha256(actual.get("log_sha256"))
        ):
            return False
    return is_sha256(payload.get("logs_sha256"))


def is_expired_receipt(comment: dict[str, Any], payload: dict[str, Any], now: int) -> bool:
    try:
        completed = iso_timestamp(payload.get("completed_at"))
        created = iso_timestamp(comment.get("created_at"))
        updated = iso_timestamp(comment.get("updated_at"))
        return (
            created == updated and abs(completed - created) <= 300
            and now > completed and now - completed > FRESHNESS_SECONDS
        )
    except EvidenceError:
        return False


def is_sha256(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def accepted_comment(comments: Any, *args: Any) -> dict[str, Any] | None:
    if not isinstance(comments, list):
        return None
    marked = []
    for comment in comments:
        if isinstance(comment, dict) and isinstance(comment.get("body"), str) and comment["body"].startswith(MARKER):
            marked.append(comment)
    if not marked:
        return None
    repository, number, base_sha, head_sha, workflow_hash, now, owner_login, stable_versions = args
    current: list[tuple[dict[str, Any], dict[str, Any]]] = []
    for comment in marked:
        payload = parse_comment(comment)
        if payload is None:
            return None
        # 旧headの証跡は履歴として残し、現在世代の重複だけを拒否する。
        if (
            payload.get("repository", "").lower() == repository.lower()
            and payload.get("pull_request") == number
            and payload.get("base_sha") == base_sha
            and payload.get("head_sha") == head_sha
            and payload.get("workflow_sha256") == workflow_hash
            and payload.get("scope_sha256") == scope_digest(workflow_hash)
        ):
            if not is_expired_receipt(comment, payload, now):
                current.append((comment, payload))
    if len(current) != 1:
        return None
    comment, payload = current[0]
    return comment if valid_payload(
        comment, payload, repository, number, base_sha, head_sha, workflow_hash, now, owner_login, stable_versions
    ) else None


def verify_from_api(
    repository: str, number: int, *, expected_base: str | None = None, expected_head: str | None = None,
    fetch: Callable[[str], Any] | None = None, now: int | None = None,
    expected_workflow_digest: str | None = None,
) -> bool:
    """Read PR/comments twice; all failures mean hosted CI should run."""
    try:
        if not workflow_scope_supported():
            return False
        if fetch is None:
            token = os.environ.get("GH_TOKEN") or gh_token()
            fetch = lambda path: api_request(token, "GET", path)
        pr_path = f"repos/{repository}/pulls/{number}"
        repo_path = f"repos/{repository}"
        comments_base = f"repos/{repository}/issues/{number}/comments?per_page=100&page="

        def read_comments() -> list[Any]:
            all_comments: list[Any] = []
            page = 1
            while True:
                page_comments = fetch(comments_base + str(page))
                if not isinstance(page_comments, list) or len(page_comments) > 100:
                    raise EvidenceError("PR comments response is incomplete")
                all_comments.extend(page_comments)
                if len(page_comments) < 100:
                    return all_comments
                page += 1

        repo_first = fetch(repo_path)
        owner_login = canonical_owner(repo_first, repository)
        pr_first = fetch(pr_path)
        base_sha, head_sha = validate_pr(pr_first, repository, number)
        if (expected_base and base_sha != expected_base) or (expected_head and head_sha != expected_head):
            return False
        compare_first = fetch(f"repos/{repository}/compare/{base_sha}...{head_sha}")
        if not base_is_merged(compare_first, base_sha):
            return False
        comments_first = read_comments()
        repo_second = fetch(repo_path)
        if canonical_owner(repo_second, repository).lower() != owner_login.lower():
            return False
        pr_second = fetch(pr_path)
        if validate_pr(pr_second, repository, number) != (base_sha, head_sha):
            return False
        compare_second = fetch(f"repos/{repository}/compare/{base_sha}...{head_sha}")
        if canonical(compare_first) != canonical(compare_second) or not base_is_merged(compare_second, base_sha):
            return False
        comments_second = read_comments()
        if canonical(comments_first) != canonical(comments_second):
            return False
        digest = expected_workflow_digest or workflow_digest()
        timestamp = int(time.time()) if now is None else now
        candidates = []
        for comment in comments_first:
            payload = parse_comment(comment)
            if payload is None:
                continue
            if (
                payload.get("repository", "").lower() == repository.lower()
                and payload.get("pull_request") == number
                and payload.get("base_sha") == base_sha
                and payload.get("head_sha") == head_sha
                and payload.get("workflow_sha256") == digest
                and payload.get("scope_sha256") == scope_digest(digest)
                and not is_expired_receipt(comment, payload, timestamp)
            ):
                candidates.append(comment)
        # Missing, stale, or ambiguous proof should take the hosted path without
        # waiting on the optional official-manifest lookup.
        if len(candidates) != 1:
            return False
        stable_versions = fetch_stable_manifest_versions()
        first = accepted_comment(
            comments_first, repository, number, base_sha, head_sha, digest, timestamp, owner_login, stable_versions
        )
        second = accepted_comment(
            comments_second, repository, number, base_sha, head_sha, digest, timestamp, owner_login, stable_versions
        )
        return first is not None and second is not None and canonical(first) == canonical(second)
    except Exception:
        return False


def output_reuse(value: bool, output_path: str | None) -> None:
    line = f"reuse={'true' if value else 'false'}\n"
    if output_path:
        with open(output_path, "a", encoding="utf-8") as output:
            output.write(line)
    else:
        print(line, end="")


def collect(repository: str, number: int, publish: bool) -> int:
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise EvidenceError("local collection requires macOS Apple Silicon")
    if not is_macos_15(macos_product_version()):
        raise EvidenceError("local collection requires macOS 15, matching the hosted macOS runner")
    require_standard_index_flags()
    if git_status():
        raise EvidenceError("working tree must be clean before collection")
    before_head = run_git("rev-parse", "HEAD")
    before_source = head_worktree_bytes_digest(before_head)
    if not workflow_scope_supported():
        raise EvidenceError("current workflow contains an unsupported macOS step")
    before_workflow = workflow_digest()
    token = gh_token()
    owner = canonical_owner(api_request(token, "GET", f"repos/{repository}"), repository)
    login = api_request(token, "GET", "user") if publish else None
    if publish and (not isinstance(login, dict) or login.get("login", "").lower() != owner.lower()):
        raise EvidenceError("publishing requires authentication as the repository owner")
    pr_path = f"repos/{repository}/pulls/{number}"
    pr_initial = api_request(token, "GET", pr_path)
    base_sha, head_sha = validate_pr(pr_initial, repository, number)
    if head_sha != before_head:
        raise EvidenceError("local HEAD does not match the pull request head")
    if run_git("merge-base", base_sha, head_sha) != base_sha:
        raise EvidenceError("local HEAD must include the exact current PR base commit")

    logs_dir = ROOT / "tmp/local-macos-evidence" / f"{number}-{head_sha[:12]}-{int(time.time())}"
    logs_dir.mkdir(parents=True, exist_ok=False)
    logs: list[dict[str, Any]] = []
    smoke_output = str(logs_dir / "plantuml-sequence.svg")

    def run_command(command_id: str, argv_template: tuple[str, ...]) -> None:
        argv = [part.format(plantuml_output=smoke_output) for part in argv_template]
        start = time.monotonic()
        command_env = command_environment()
        result = subprocess.run(argv, cwd=ROOT, env=command_env, text=True, capture_output=True)
        duration = int((time.monotonic() - start) * 1000)
        output = result.stdout + result.stderr
        if command_id == "plantuml-render" and result.returncode == 0:
            svg = Path(smoke_output).read_text(encoding="utf-8")
            if "<svg" not in svg:
                result.returncode = 1
                output += "\nPlantUML smoke output did not contain SVG\n"
        log_path = logs_dir / f"{len(logs) + 1:02d}-{command_id}.log"
        log_path.write_text(output, encoding="utf-8")
        logs.append({
            "id": command_id, "argv": argv_template, "exit_code": result.returncode,
            "duration_ms": duration, "log_sha256": sha256_bytes(output.encode()),
        })
        if result.returncode != 0:
            raise EvidenceError(f"local command failed: {command_id}; log: {log_path}")

    for command_id, argv_template in COMMANDS:
        if command_id in PREPARATION_COMMANDS:
            run_command(command_id, argv_template)
    stable_versions_before = fetch_stable_manifest_versions()
    versions_before = tool_versions()
    validate_pinned_tools(versions_before)
    validate_stable_rust_tools(versions_before, stable_versions_before)
    for command_id, argv_template in COMMANDS:
        if command_id not in PREPARATION_COMMANDS:
            run_command(command_id, argv_template)
    stable_versions_after = fetch_stable_manifest_versions()
    versions_after = tool_versions()
    validate_pinned_tools(versions_after)
    validate_stable_rust_tools(versions_after, stable_versions_after)
    if versions_before != versions_after or stable_versions_before != stable_versions_after:
        raise EvidenceError("tool versions changed during local checks")
    versions = versions_before
    completed_at = utc_now()
    aggregate = canonical(logs)
    (logs_dir / "commands.json").write_bytes(aggregate)
    logs_sha = sha256_bytes(aggregate + b"\n".join((logs_dir / f"{i:02d}-{item['id']}.log").read_bytes() for i, item in enumerate(logs, 1)))
    require_standard_index_flags()
    after_head = run_git("rev-parse", "HEAD")
    after_source = head_worktree_bytes_digest(after_head)
    if (
        git_status() or after_head != before_head or after_source != before_source
        or workflow_digest() != before_workflow or not workflow_scope_supported()
    ):
        raise EvidenceError("working tree bytes, HEAD, or workflow changed during collection")
    pr_final = api_request(token, "GET", pr_path)
    if validate_pr(pr_final, repository, number) != (base_sha, head_sha):
        raise EvidenceError("pull request base or head changed during collection")
    compare = api_request(token, "GET", f"repos/{repository}/compare/{base_sha}...{head_sha}")
    if not base_is_merged(compare, base_sha):
        raise EvidenceError("pull request base is not included in the tested head")
    payload = {
        "schema": SCHEMA, "repository": repository, "pull_request": number,
        "base_sha": base_sha, "head_sha": head_sha, "workflow_sha256": before_workflow,
        "scope_sha256": scope_digest(before_workflow), "completed_at": completed_at,
        "platform": "macos-arm64", "tools": versions, "commands": logs,
        "environment": SCOPE_PARAMETERS, "logs_sha256": logs_sha,
    }
    proof_path = logs_dir / "evidence.json"
    proof_path.write_bytes(canonical(payload))
    print(f"Local macOS checks passed. Logs and attestation: {proof_path}")
    if publish:
        # 旧headの証跡は残し、現在世代だけ重複投稿を防ぐ。
        comments = read_comments_api(token, repository, number)
        generation_keys = ("repository", "pull_request", "base_sha", "head_sha", "workflow_sha256", "scope_sha256")
        current_comments = []
        for comment in comments:
            if not isinstance(comment, dict) or not isinstance(comment.get("body"), str) or not comment["body"].startswith(MARKER):
                continue
            existing = parse_comment(comment)
            if existing is None:
                raise EvidenceError("malformed comment uses the evidence marker")
            same_generation = all(existing.get(key) == payload[key] for key in generation_keys)
            if same_generation:
                user = comment.get("user")
                if not isinstance(user, dict) or user.get("login", "").lower() != owner.lower() or comment.get("author_association") != "OWNER":
                    raise EvidenceError("ambiguous non-owner comment uses the current evidence marker")
                if is_expired_receipt(comment, existing, int(time.time())):
                    continue
                if verify_from_api(
                    repository, number, expected_base=base_sha, expected_head=head_sha,
                    fetch=lambda path: api_request(token, "GET", path),
                    expected_workflow_digest=before_workflow,
                ):
                    print("A fresh attestation for this PR generation is already published.")
                    return 0
                current_comments.append(comment)
        if current_comments:
            raise EvidenceError("a proof already exists for this PR generation")
        with tempfile.NamedTemporaryFile("w", encoding="utf-8", suffix=".json", delete=False) as body_file:
            json.dump({"body": MARKER + "\n" + canonical(payload).decode()}, body_file, separators=(",", ":"))
            body_path = Path(body_file.name)
        try:
            result = subprocess.run(
                ["gh", "api", f"repos/{repository}/issues/{number}/comments", "--method", "POST", "--input", str(body_path)],
                cwd=ROOT, check=True, text=True, capture_output=True,
            )
        finally:
            body_path.unlink(missing_ok=True)
        posted = json.loads(result.stdout)
        posted_user = posted.get("user") if isinstance(posted, dict) else None
        if (
            not isinstance(posted_user, dict) or posted_user.get("login", "").lower() != owner.lower()
            or posted.get("author_association") != "OWNER"
        ):
            raise EvidenceError("posted attestation author is not the repository owner")
        posted_comment_id = posted.get("id")
        if not isinstance(posted_comment_id, int) or not verify_from_api(
            repository, number, expected_base=base_sha, expected_head=head_sha,
            fetch=lambda path: api_request(token, "GET", path),
            expected_workflow_digest=before_workflow,
        ):
            raise EvidenceError("published attestation did not pass an independent read")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify", action="store_true", help="set reuse output from current PR proof")
    parser.add_argument("--repo", default=os.environ.get("GITHUB_REPOSITORY", ""))
    parser.add_argument("--pr", type=int, default=int(os.environ.get("PR_NUMBER", "0") or 0))
    parser.add_argument("--base", default=os.environ.get("PR_BASE_SHA", ""))
    parser.add_argument("--head", default=os.environ.get("PR_HEAD_SHA", ""))
    parser.add_argument("--workflow", default=str(WORKFLOW))
    parser.add_argument("--output", default=os.environ.get("GITHUB_OUTPUT"))
    parser.add_argument("--publish", action="store_true", help="publish proof using existing gh auth")
    args = parser.parse_args()
    if args.verify:
        output_reuse(False, args.output)
        try:
            workflow_path = Path(args.workflow)
            if not workflow_path.is_absolute():
                workflow_path = ROOT / workflow_path
            valid = bool(
                args.repo and args.pr > 0 and SHA_RE.fullmatch(args.base) and SHA_RE.fullmatch(args.head)
                and verify_from_api(
                    args.repo, args.pr, expected_base=args.base, expected_head=args.head,
                    expected_workflow_digest=workflow_digest(workflow_path),
                )
            )
            if valid and args.output:
                with open(args.output, "a", encoding="utf-8") as output:
                    output.write("reuse=true\n")
            elif valid:
                print("reuse=true")
        except Exception:
            pass
        return 0
    try:
        repository = args.repo
        if not repository:
            repository = run_git("remote", "get-url", "origin").rstrip("/").removesuffix(".git").rsplit(":", 1)[-1]
        collect(repository, args.pr, args.publish)
        return 0
    except (EvidenceError, OSError, subprocess.CalledProcessError, ValueError) as exc:
        print(f"local macOS evidence: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
