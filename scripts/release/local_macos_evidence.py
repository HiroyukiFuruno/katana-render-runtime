#!/usr/bin/env python3
"""Collect and optionally publish owner-asserted local macOS CI evidence."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any, Callable, NamedTuple
from urllib import error, request

try:
    import tomllib
except ModuleNotFoundError:
    tomllib = None


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/test-and-build.yml"
SUPPORTED_WORKFLOW_SHA256 = "768c0c48fcb06863ba5c10f6fb52e2b4b180f49a0a4766aa0b38956b5bac743e"
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
    "CARGO_HOME": "<fresh-empty-directory-per-collection>",
    "CARGO_ENCODED_RUSTFLAGS": "-D\x1fwarnings",
    "CARGO_INCREMENTAL": "0",
    "CARGO_TARGET_DIR": "<fresh-empty-directory-per-collection>",
    "CARGO_TERM_COLOR": "always",
    "CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER": "/usr/bin/env",
    "RTK": "",
    "RUSTFLAGS": "-D warnings",
    "RUSTC": "rustc",
    "RUSTC_WRAPPER": "",
    "RUSTC_WORKSPACE_WRAPPER": "",
    "BUN_INSTALL_CACHE_DIR": "<fresh-empty-directory-per-collection>",
    "TEST_THREADS": "1",
}
JAVA_VENDOR = "Eclipse Adoptium"
PREPARATION_COMMANDS = {"bun-install", "graphviz-install", "plantuml-install"}
RUST_STABLE_MANIFEST_URL = "https://static.rust-lang.org/dist/channel-rust-stable.toml"
MAX_RUST_STABLE_MANIFEST_BYTES = 8 * 1024 * 1024
JUST_RELEASE_REPOSITORY = "casey/just"
MAX_JUST_ASSET_BYTES = 8 * 1024 * 1024
MAX_JUST_BINARY_BYTES = 32 * 1024 * 1024
MAX_JUST_ARCHIVE_ENTRIES = 128
MAX_JUST_ARCHIVE_CONTENT_BYTES = 16 * 1024 * 1024
HOST_ENVIRONMENT_KEYS = (
    "PATH", "HOME", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME", "LANG", "LC_ALL",
    "LC_CTYPE", "LC_MESSAGES", "LC_COLLATE", "LC_NUMERIC", "LC_TIME", "TERM",
)

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
class JustBinding(NamedTuple):
    path: Path
    version: str
    release_tag: str
    asset_name: str
    asset_url: str
    asset_sha256: str
    binary_sha256: str


class CommandBinding(NamedTuple):
    path: str
    executables: dict[str, tuple[str, str]]

    def proof_value(self) -> str:
        return canonical({"path": self.path, "executables": self.executables}).decode("utf-8")


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


def clean_existing_node_modules() -> None:
    node_modules = ROOT / "node_modules"
    try:
        mode = node_modules.lstat().st_mode
    except FileNotFoundError:
        return
    try:
        if stat.S_ISDIR(mode) and not stat.S_ISLNK(mode):
            shutil.rmtree(node_modules)
        else:
            node_modules.unlink()
    except OSError as exc:
        raise EvidenceError("cannot remove ignored node_modules before dependency install") from exc


def reject_git_replace_refs() -> None:
    environment = dict(os.environ)
    environment["GIT_NO_REPLACE_OBJECTS"] = "1"
    ref_bases = {"refs/replace/"}
    configured_base = environment.get("GIT_REPLACE_REF_BASE")
    if configured_base:
        ref_bases.add(configured_base.rstrip("/") + "/")
    for ref_base in ref_bases:
        result = subprocess.run(
            [git_program(), "--no-replace-objects", "for-each-ref", "--format=%(refname)", ref_base],
            cwd=ROOT, check=True, text=True, capture_output=True, env=environment,
        )
        if result.stdout.strip():
            raise EvidenceError("Git replacement refs invalidate local proof")


def run_git(*args: str) -> str:
    reject_git_replace_refs()
    environment = dict(os.environ)
    environment["GIT_NO_REPLACE_OBJECTS"] = "1"
    result = subprocess.run(
        [git_program(), "--no-replace-objects", *args], cwd=ROOT, check=True, text=True,
        capture_output=True, env=environment,
    )
    return result.stdout.strip()


def git_program() -> str:
    return "/usr/bin/git" if platform.system() == "Darwin" else "git"


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
        reject_git_replace_refs()
        tree = subprocess.run(
            ["git", "--no-replace-objects", "ls-tree", "-r", "-z", "--full-tree", expected_head],
            cwd=ROOT, check=True, capture_output=True,
            env={**os.environ, "GIT_NO_REPLACE_OBJECTS": "1"},
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
        ["/usr/bin/sw_vers", "-productVersion"], cwd=ROOT, check=True, text=True, capture_output=True,
    )
    version = result.stdout.strip()
    if not version:
        raise EvidenceError("macOS product version is unavailable")
    return version


def is_macos_15(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"15\.\d+(?:\.\d+)?", value.strip()) is not None


def java_home_from_settings(output: str) -> str:
    if not java_is_21(output):
        raise EvidenceError("local proof requires Java 21, matching the macOS CI setup")
    java_vendor_from_settings(output)
    match = re.search(r"(?m)^\s*java\.home\s*=\s*(\S.*?)\s*$", output)
    if match is None:
        raise EvidenceError("Java runtime did not report java.home")
    home = Path(match.group(1)).expanduser()
    if not home.is_absolute() or not home.is_dir():
        raise EvidenceError("Java runtime reported an invalid java.home")
    return str(home.resolve())


def java_vendor_from_settings(output: str) -> str:
    vendors = re.findall(r"(?m)^\s*java\.vendor\s*=\s*(.*?)\s*$", output)
    if vendors != [JAVA_VENDOR]:
        raise EvidenceError("local proof requires Temurin (Eclipse Adoptium), matching the macOS CI setup")
    return vendors[0]


def detect_java_home(environment: dict[str, str]) -> str:
    result = subprocess.run(
        ("java", "-XshowSettings:properties", "-version"), cwd=ROOT,
        env=environment, text=True, capture_output=True, check=True,
    )
    output = result.stdout + result.stderr
    home = java_home_from_settings(output)
    java = Path(home) / "bin/java"
    if not java.is_file():
        raise EvidenceError("validated Java home has no java executable")
    libjvm = (
        Path(home) / "lib/server/libjvm.dylib",
        Path(home) / "jre/lib/server/libjvm.dylib",
        Path(home) / "bin/server/libjvm.dylib",
    )
    if not any(candidate.is_file() for candidate in libjvm):
        raise EvidenceError("validated Java home has no PlantUML JVM library")
    explicit = subprocess.run(
        (str(java), "-XshowSettings:properties", "-version"), cwd=ROOT, env=environment,
        text=True, capture_output=True, check=True,
    )
    explicit_output = explicit.stdout + explicit.stderr
    if not java_is_21(explicit_output):
        raise EvidenceError("JAVA_HOME does not identify Java 21")
    if java_home_from_settings(explicit_output) != home:
        raise EvidenceError("JAVA_HOME does not identify the selected Java runtime")
    return home


def account_home() -> Path:
    try:
        import pwd

        home = Path(pwd.getpwuid(os.getuid()).pw_dir).resolve(strict=True)
    except (KeyError, OSError) as exc:
        raise EvidenceError("OS account home is unavailable") from exc
    if not home.is_dir():
        raise EvidenceError("OS account home is invalid")
    return home


def trusted_executable(path: Path, roots: tuple[Path, ...]) -> Path:
    try:
        resolved = path.resolve(strict=True)
        metadata = resolved.stat()
    except OSError as exc:
        raise EvidenceError(f"trusted tool is unavailable: {path.name}") from exc
    if (not stat.S_ISREG(metadata.st_mode) or not os.access(resolved, os.X_OK)
            or not any(resolved == root or root in resolved.parents for root in roots)
            or ROOT == resolved or ROOT in resolved.parents):
        raise EvidenceError(f"tool is outside the trusted installation roots: {path.name}")
    return resolved


def command_binding(environment: dict[str, str], just_binding: JustBinding) -> CommandBinding:
    home = account_home()
    brew_root = Path("/opt/homebrew").resolve(strict=True)
    system_roots = (Path("/usr/bin"), Path("/bin"), Path("/usr/sbin"), Path("/sbin"))
    python_dir = Path(sys.executable).resolve(strict=True).parent
    rustup_home = Path(environment["RUSTUP_HOME"]).resolve(strict=True)
    default_rustup_home = (home / ".rustup").resolve()
    if rustup_home != default_rustup_home and default_rustup_home not in rustup_home.parents:
        raise EvidenceError("custom RUSTUP_HOME is outside the account installation")
    rustup = trusted_executable(home / ".cargo/bin/rustup", (home / ".cargo/bin",))
    bootstrap_env = dict(environment)
    bootstrap_env["PATH"] = "/usr/bin:/bin:/usr/sbin:/sbin"
    bootstrap_env["RUSTUP_HOME"] = str(rustup_home)
    rust_root = rustup_home / "toolchains/stable-aarch64-apple-darwin"
    rust_root = rust_root.resolve(strict=True)
    rust_paths = {}
    for name in ("cargo", "rustc", "cargo-clippy", "clippy-driver", "rustfmt"):
        selected = Path(subprocess.run(
            (str(rustup), "which", "--toolchain", "stable-aarch64-apple-darwin", name),
            cwd=ROOT, env=bootstrap_env, text=True, capture_output=True, check=True,
        ).stdout.strip())
        rust_paths[name] = trusted_executable(selected, (rust_root,))
    rust_bin = rust_paths["rustc"].parent
    if any(path.parent != rust_bin for path in rust_paths.values()):
        raise EvidenceError("stable Rust components do not share one toolchain directory")

    brew_bin = trusted_executable(brew_root / "bin/brew", (brew_root,))
    bun_bin = trusted_executable(brew_root / "bin/bun", (brew_root,))
    dot_bin = trusted_executable(brew_root / "bin/dot", (brew_root,))
    bash_bin = trusted_executable(brew_root / "bin/bash", (brew_root,))
    python_bin = trusted_executable(Path(sys.executable), (python_dir,))
    java_home_result = subprocess.run(
        ("/usr/libexec/java_home", "-v", "21"), cwd=ROOT, env=bootstrap_env,
        text=True, capture_output=True, check=True,
    )
    java_home = Path(java_home_result.stdout.strip()).resolve(strict=True)
    java_root = Path("/Library/Java/JavaVirtualMachines").resolve(strict=True)
    if java_root not in java_home.parents:
        raise EvidenceError("selected Java installation is outside the trusted system root")
    java_bin = trusted_executable(java_home / "bin/java", (java_root,))

    paths = [str(just_binding.path.parent), str(rust_bin), str(java_home / "bin"), str(python_dir),
             str(brew_root / "bin"), *(str(root) for root in system_roots)]
    command_path = os.pathsep.join(dict.fromkeys(paths))
    git_candidate = shutil.which("git", path=command_path)
    if git_candidate is None:
        raise EvidenceError("trusted Git executable is unavailable")
    git_bin = trusted_executable(Path(git_candidate), (brew_root, *system_roots))
    source_git = trusted_executable(Path("/usr/bin/git"), system_roots)
    sw_vers_bin = trusted_executable(Path("/usr/bin/sw_vers"), system_roots)
    uname_bin = trusted_executable(Path("/usr/bin/uname"), system_roots)
    executables = {
        "just": (str(just_binding.path.resolve(strict=True)), just_binding.binary_sha256),
        "cargo": (str(rust_paths["cargo"]), sha256_bytes(rust_paths["cargo"].read_bytes())),
        "rustc": (str(rust_paths["rustc"]), sha256_bytes(rust_paths["rustc"].read_bytes())),
        "cargo-clippy": (str(rust_paths["cargo-clippy"]), sha256_bytes(rust_paths["cargo-clippy"].read_bytes())),
        "clippy-driver": (str(rust_paths["clippy-driver"]), sha256_bytes(rust_paths["clippy-driver"].read_bytes())),
        "rustfmt": (str(rust_paths["rustfmt"]), sha256_bytes(rust_paths["rustfmt"].read_bytes())),
        "brew": (str(brew_bin), sha256_bytes(brew_bin.read_bytes())),
        "bun": (str(bun_bin), sha256_bytes(bun_bin.read_bytes())),
        "dot": (str(dot_bin), sha256_bytes(dot_bin.read_bytes())),
        "bash": (str(bash_bin), sha256_bytes(bash_bin.read_bytes())),
        "python3": (str(python_bin), sha256_bytes(python_bin.read_bytes())),
        "java": (str(java_bin), sha256_bytes(java_bin.read_bytes())),
        "git": (str(git_bin), sha256_bytes(git_bin.read_bytes())),
        "source_git": (str(source_git), sha256_bytes(source_git.read_bytes())),
        "sw_vers": (str(sw_vers_bin), sha256_bytes(sw_vers_bin.read_bytes())),
        "uname": (str(uname_bin), sha256_bytes(uname_bin.read_bytes())),
    }
    return CommandBinding(command_path, executables)


def command_environment(
    cargo_target_dir: Path, isolated_cargo_home: Path, bun_cache_dir: Path,
    binding: CommandBinding | None = None,
) -> dict[str, str]:
    parent = os.environ
    unsupported = {
        name for name in parent
        if name in {"RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"}
        or name.startswith("CARGO_BUILD_RUSTC")
        or (name.startswith("CARGO_TARGET_") and name != "CARGO_TARGET_DIR")
    }
    if unsupported:
        raise EvidenceError("custom Rust compiler or target linker environment is unsupported")
    configs: set[Path] = set()
    for directory in (ROOT, *ROOT.parents):
        configs.update((directory / ".cargo/config", directory / ".cargo/config.toml"))
    parent_cargo_home = Path(parent.get("CARGO_HOME", str(Path(parent.get("HOME", "~")) / ".cargo"))).expanduser()
    if not parent_cargo_home.is_absolute():
        parent_cargo_home = ROOT / parent_cargo_home
    parent_cargo_home = parent_cargo_home.resolve()
    configs.update((parent_cargo_home / "config", parent_cargo_home / "config.toml"))
    for config in sorted(configs):
        if not config.is_file():
            continue
        if tomllib is None:
            raise EvidenceError("cannot validate Cargo config without TOML support")
        try:
            parsed = tomllib.loads(config.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, ValueError) as exc:
            raise EvidenceError("Cargo config cannot be validated") from exc
        if "env" in parsed:
            raise EvidenceError("Cargo config [env] injection is unsupported")
        if "source" in parsed:
            raise EvidenceError("Cargo config source replacement is unsupported")
        if parsed.get("paths"):
            raise EvidenceError("Cargo config paths overrides are unsupported")
        if "include" in parsed:
            raise EvidenceError("Cargo config includes are unsupported")
        aliases = parsed.get("alias")
        if isinstance(aliases, dict) and any(name in aliases for name in ("clippy", "fmt")):
            raise EvidenceError("Cargo config aliases for clippy/fmt are unsupported")
        if "profile" in parsed:
            raise EvidenceError("Cargo config profile overrides are unsupported")
        build_config = parsed.get("build")
        if isinstance(build_config, dict) and "warnings" in build_config:
            raise EvidenceError("Cargo config build.warnings override is unsupported")
        resolver_config = parsed.get("resolver")
        if isinstance(resolver_config, dict) and "lockfile-path" in resolver_config:
            raise EvidenceError("Cargo config resolver.lockfile-path is unsupported")
        target_config = parsed.get("target")
        if isinstance(target_config, dict) and any(
            isinstance(target, dict) and (
                "linker" in target or any(isinstance(value, dict) for value in target.values())
            )
            for target in target_config.values()
        ):
            # WHY: cfg式やlinks設定がnative targetの実行内容を変える経路を見落とさない。
            raise EvidenceError("Cargo config target linker or links override is unsupported")
    home = account_home()
    environment = {name: parent[name] for name in HOST_ENVIRONMENT_KEYS if name in parent
                   and name in {"TMPDIR", "LANG", "LC_ALL", "LC_CTYPE", "LC_MESSAGES", "LC_COLLATE", "LC_NUMERIC", "LC_TIME", "TERM"}}
    environment["HOME"] = str(home)
    environment["RUSTUP_HOME"] = str(home / ".rustup")
    environment.update(SCOPE_PARAMETERS)
    environment["CARGO_HOME"] = str(isolated_cargo_home)
    environment["CARGO_TARGET_DIR"] = str(cargo_target_dir)
    environment["BUN_INSTALL_CACHE_DIR"] = str(bun_cache_dir)
    if binding is None:
        environment["PATH"] = "/usr/bin:/bin:/usr/sbin:/sbin"
    else:
        environment["PATH"] = binding.path
        environment["JAVA_HOME"] = str(Path(binding.executables["java"][0]).parent.parent)
        java_version = subprocess.run(
            (binding.executables["java"][0], "-XshowSettings:properties", "-version"),
            cwd=ROOT, env=environment, text=True, capture_output=True, check=True,
        )
        java_home = java_home_from_settings(java_version.stdout + java_version.stderr)
        if java_home != environment["JAVA_HOME"]:
            raise EvidenceError("JAVA_HOME does not identify the selected runtime")
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


def materialize_trusted_just(
    release: Any, archive: bytes, destination: Path, environment: dict[str, str],
) -> JustBinding:
    if not isinstance(release, dict):
        raise EvidenceError("official just release metadata is invalid")
    tag = release.get("tag_name")
    if (not isinstance(tag, str) or re.fullmatch(r"\d+\.\d+\.\d+", tag) is None
            or release.get("draft") is not False or release.get("prerelease") is not False):
        raise EvidenceError("official just release identity is invalid")
    asset_name = f"just-{tag}-aarch64-apple-darwin.tar.gz"
    asset_url = f"https://github.com/{JUST_RELEASE_REPOSITORY}/releases/download/{tag}/{asset_name}"
    assets = release.get("assets")
    if not isinstance(assets, list):
        raise EvidenceError("official just release assets are missing")
    matches = [asset for asset in assets if isinstance(asset, dict) and asset.get("name") == asset_name]
    if len(matches) != 1:
        raise EvidenceError("official release asset is missing or ambiguous")
    asset = matches[0]
    digest = asset.get("digest")
    size = asset.get("size")
    if (asset.get("browser_download_url") != asset_url
            or not isinstance(digest, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", digest) is None
            or type(size) is not int or size < 1 or size > MAX_JUST_ASSET_BYTES
            or len(archive) != size):
        raise EvidenceError("official release asset metadata is invalid")
    asset_sha256 = sha256_bytes(archive)
    if digest != f"sha256:{asset_sha256}":
        raise EvidenceError("official just asset digest does not match the downloaded bytes")
    try:
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as bundle:
            members = bundle.getmembers()
            if not members or len(members) > MAX_JUST_ARCHIVE_ENTRIES:
                raise EvidenceError("official just archive entry count is invalid")
            total_size = 0
            just_members = []
            names = set()
            for item in members:
                member_path = PurePosixPath(item.name)
                if (not item.name or member_path.is_absolute() or ".." in member_path.parts
                        or member_path.as_posix() != item.name
                        or item.name in names or not (item.isfile() or item.isdir())):
                    raise EvidenceError("official just archive contains an unsafe member")
                names.add(item.name)
                if item.isfile():
                    if item.size < 0 or item.size > MAX_JUST_BINARY_BYTES:
                        raise EvidenceError("official just archive member size is invalid")
                    total_size += item.size
                    if item.name == "just":
                        just_members.append(item)
            if total_size > MAX_JUST_ARCHIVE_CONTENT_BYTES or len(just_members) != 1:
                raise EvidenceError("official just archive does not contain one regular just binary")
            member = just_members[0]
            if member.size < 1 or member.size > MAX_JUST_BINARY_BYTES:
                raise EvidenceError("official just binary size is invalid")
            stream = bundle.extractfile(member)
            if stream is None:
                raise EvidenceError("official just binary cannot be read")
            binary = stream.read(MAX_JUST_BINARY_BYTES + 1)
    except (OSError, tarfile.TarError) as exc:
        raise EvidenceError("official just archive is invalid") from exc
    if len(binary) != member.size:
        raise EvidenceError("official just binary is truncated")

    tool_directory = destination / "trusted-just"
    destination.mkdir(parents=True, exist_ok=True)
    tool_directory.mkdir(mode=0o700)
    executable = tool_directory / "just"
    executable.write_bytes(binary)
    executable.chmod(0o700)
    result = subprocess.run(
        [str(executable), "--version"], env=environment,
        text=True, capture_output=True, check=True,
    )
    versions = (result.stdout + result.stderr).strip().splitlines()
    expected_version = f"just {tag}"
    if versions != [expected_version]:
        raise EvidenceError("official just binary version does not match its release tag")
    return JustBinding(
        path=executable.resolve(), version=expected_version, release_tag=tag,
        asset_name=asset_name, asset_url=asset_url, asset_sha256=asset_sha256,
        binary_sha256=sha256_bytes(binary),
    )


def download_trusted_just(
    token: str, destination: Path, environment: dict[str, str],
) -> JustBinding:
    release = api_request(token, "GET", f"repos/{JUST_RELEASE_REPOSITORY}/releases/latest")
    if not isinstance(release, dict) or not isinstance(release.get("assets"), list):
        raise EvidenceError("official just release metadata is invalid")
    tag = release.get("tag_name")
    if not isinstance(tag, str) or re.fullmatch(r"\d+\.\d+\.\d+", tag) is None:
        raise EvidenceError("official just release tag is invalid")
    asset_name = f"just-{tag}-aarch64-apple-darwin.tar.gz"
    matches = [asset for asset in release["assets"] if isinstance(asset, dict) and asset.get("name") == asset_name]
    if len(matches) != 1:
        raise EvidenceError("official just Apple Silicon release asset is missing or ambiguous")
    asset = matches[0]
    asset_url = f"https://github.com/{JUST_RELEASE_REPOSITORY}/releases/download/{tag}/{asset_name}"
    if asset.get("browser_download_url") != asset_url:
        raise EvidenceError("official just asset URL is invalid")
    try:
        req = request.Request(asset_url, headers={"Accept": "application/octet-stream", "User-Agent": "katana-render-runtime-local-macos-evidence"})
        with request.urlopen(req, timeout=60) as response:
            final_url = response.geturl()
            final_host = request.urlparse(final_url).hostname
            if final_host not in {"github.com", "release-assets.githubusercontent.com", "objects.githubusercontent.com"}:
                raise EvidenceError("official just asset redirected to an untrusted host")
            archive = response.read(MAX_JUST_ASSET_BYTES + 1)
    except EvidenceError:
        raise
    except (error.URLError, error.HTTPError, TimeoutError, OSError) as exc:
        raise EvidenceError("official just asset download failed") from exc
    if len(archive) > MAX_JUST_ASSET_BYTES:
        raise EvidenceError("official just asset exceeds its size limit")
    return materialize_trusted_just(release, archive, destination, environment)


def prepare_command_environment(
    token: str, isolated_inputs_dir: Path, cargo_target_dir: Path, cargo_home: Path,
    bun_cache_dir: Path,
) -> tuple[dict[str, str], JustBinding, CommandBinding, dict[str, str]]:
    try:
        bootstrap = command_environment(cargo_target_dir, cargo_home, bun_cache_dir)
        just_binding = download_trusted_just(token, isolated_inputs_dir, bootstrap)
        binding = command_binding(bootstrap, just_binding)
        environment = command_environment(cargo_target_dir, cargo_home, bun_cache_dir, binding)
        return bootstrap, just_binding, binding, environment
    except BaseException:
        shutil.rmtree(isolated_inputs_dir, ignore_errors=True)
        raise


def assert_trusted_just(binding: JustBinding) -> None:
    try:
        path = binding.path
        metadata = path.lstat()
        if (path.is_symlink() or not stat.S_ISREG(metadata.st_mode)
                or path.resolve(strict=True) != path
                or sha256_bytes(path.read_bytes()) != binding.binary_sha256):
            raise EvidenceError("trusted just executable changed after authentication")
    except OSError as error:
        raise EvidenceError("trusted just executable path is unavailable") from error


def assert_command_binding(binding: CommandBinding, just_binding: JustBinding) -> None:
    assert_trusted_just(just_binding)
    for name, (raw_path, expected_hash) in binding.executables.items():
        path = Path(raw_path)
        try:
            effective = str(path) if name == "source_git" else shutil.which(name, path=binding.path)
            if (path.resolve(strict=True) != path or not stat.S_ISREG(path.stat().st_mode)
                    or sha256_bytes(path.read_bytes()) != expected_hash
                    or effective is None or Path(effective).resolve(strict=True) != path):
                raise EvidenceError(f"trusted command executable changed: {name}")
        except OSError as exc:
            raise EvidenceError(f"trusted command executable is unavailable: {name}") from exc


def bind_command(argv: tuple[str, ...], binding: JustBinding) -> list[str]:
    if argv[0] != "just":
        return list(argv)
    assert_trusted_just(binding)
    return [str(binding.path), *argv[1:]]


def cargo_component_versions(environment: dict[str, str]) -> dict[str, str]:
    found: dict[str, str] = {}
    for name, command in (
        ("clippy", ("cargo", "clippy", "--version")),
        ("rustfmt", ("cargo", "fmt", "--version")),
    ):
        result = subprocess.run(
            command, cwd=ROOT, env=environment, text=True, capture_output=True, check=True,
        )
        versions = (result.stdout + result.stderr).strip().splitlines()
        if not versions or not versions[0].strip():
            raise EvidenceError(f"tool version is empty: {name}")
        found[name] = versions[0].strip()
    return found


def tool_versions(
    cargo_target_dir: Path, cargo_home: Path, bun_cache_dir: Path, just_binding: JustBinding,
    binding: CommandBinding,
) -> dict[str, str]:
    assert_command_binding(binding, just_binding)
    environment = command_environment(cargo_target_dir, cargo_home, bun_cache_dir, binding)
    commands = {
        "macos": ("/usr/bin/sw_vers", "-productVersion"),
        "architecture": ("/usr/bin/uname", "-m"),
        "rustc": ("rustc", "--version"),
        "cargo": ("cargo", "--version"),
        "java": ("java", "-version"),
        "bun": ("bun", "--version"),
        "just": (str(just_binding.path), "--version"),
        "brew": ("brew", "--version"),
        "graphviz": ("dot", "-V"),
    }
    found: dict[str, str] = {}
    for name, command in commands.items():
        result = subprocess.run(
            command, cwd=ROOT, env=environment, text=True, capture_output=True, check=True,
        )
        value = (result.stdout + result.stderr).strip().splitlines()
        if not value or not value[0].strip():
            raise EvidenceError(f"tool version is empty: {name}")
        found[name] = value[0].strip()
    found.update(cargo_component_versions(environment))
    assert_trusted_just(just_binding)
    found.update({
        "just_path": str(just_binding.path),
        "just_release": just_binding.release_tag,
        "just_asset": just_binding.asset_name,
        "just_asset_url": just_binding.asset_url,
        "just_asset_sha256": just_binding.asset_sha256,
        "just_binary_sha256": just_binding.binary_sha256,
        "command_binding": binding.proof_value(),
    })
    if found["just"] != just_binding.version:
        raise EvidenceError("trusted just version changed during collection")
    if found["architecture"] != "arm64":
        raise EvidenceError("local proof requires Apple Silicon (arm64)")
    if platform.system() != "Darwin":
        raise EvidenceError("local proof requires macOS")
    if not is_macos_15(found["macos"]):
        raise EvidenceError("local proof requires macOS 15, matching the hosted macOS runner")
    rust_verbose = subprocess.run(
        ("rustc", "-vV"), cwd=ROOT, env=environment,
        text=True, capture_output=True, check=True,
    ).stdout
    host_target = re.search(r"(?m)^host: (\S+)$", rust_verbose)
    if host_target is None or host_target.group(1) != SCOPE_PARAMETERS["CARGO_BUILD_TARGET"]:
        raise EvidenceError("local Rust host target is not native Apple Silicon macOS")
    found["rust_host"] = host_target.group(1)
    found["java_home"] = environment["JAVA_HOME"]
    java_properties = subprocess.run(
        (str(Path(environment["JAVA_HOME"]) / "bin/java"), "-XshowSettings:properties", "-version"),
        cwd=ROOT, env=environment, text=True, capture_output=True, check=True,
    )
    found["java_vendor"] = java_vendor_from_settings(java_properties.stdout + java_properties.stderr)
    validate_pinned_tools(found)
    return found


def java_is_21(value: Any) -> bool:
    return isinstance(value, str) and re.search(
        r'\bversion\s+["\']?21(?:[.+"\'\s]|$)', value, re.IGNORECASE
    ) is not None


def validate_pinned_tools(versions: Any) -> None:
    if not isinstance(versions, dict) or not java_is_21(versions.get("java")):
        raise EvidenceError("local proof requires Java 21, matching the macOS CI setup")
    if versions.get("java_vendor") != JAVA_VENDOR:
        raise EvidenceError("local proof requires Temurin (Eclipse Adoptium), matching the macOS CI setup")
    java_home = versions.get("java_home")
    if not isinstance(java_home, str) or not Path(java_home).is_absolute():
        raise EvidenceError("local proof requires a validated absolute Java home")
    if versions.get("bun") != "1.4.2":
        raise EvidenceError("local proof requires Bun 1.4.2, matching the macOS CI setup")
    just = versions.get("just")
    version = re.fullmatch(r"just (\d+\.\d+\.\d+)", just) if isinstance(just, str) else None
    if version is None:
        raise EvidenceError("local proof requires an authenticated just release")
    tag = version.group(1)
    asset_name = f"just-{tag}-aarch64-apple-darwin.tar.gz"
    if (versions.get("just_release") != tag or versions.get("just_asset") != asset_name
            or versions.get("just_asset_url") != f"https://github.com/{JUST_RELEASE_REPOSITORY}/releases/download/{tag}/{asset_name}"
            or not is_sha256(versions.get("just_asset_sha256"))
            or not is_sha256(versions.get("just_binary_sha256"))):
        raise EvidenceError("just release digest or asset identity is invalid")
    just_path = versions.get("just_path")
    if (not isinstance(just_path, str) or not Path(just_path).is_absolute()
            or Path(just_path).name != "just" or Path(just_path).parent.name != "trusted-just"
            or not any(part.startswith("krr-local-macos-inputs-") for part in Path(just_path).parts)):
        raise EvidenceError("just executable path is not the isolated authenticated tool")
    raw_binding = versions.get("command_binding")
    try:
        command = json.loads(raw_binding) if isinstance(raw_binding, str) else None
    except json.JSONDecodeError:
        command = None
    required_executables = {
        "just", "cargo", "rustc", "cargo-clippy", "clippy-driver", "rustfmt", "brew", "bun",
        "dot", "bash", "python3", "java", "git", "source_git", "sw_vers", "uname",
    }
    if (not isinstance(command, dict) or set(command) != {"path", "executables"}
            or not isinstance(command.get("path"), str) or not command["path"]
            or not isinstance(command.get("executables"), dict)
            or set(command["executables"]) != required_executables):
        raise EvidenceError("shared command path binding is missing or invalid")
    for name, entry in command["executables"].items():
        if (not isinstance(entry, list) or len(entry) != 2 or not isinstance(entry[0], str)
                or not Path(entry[0]).is_absolute() or not is_sha256(entry[1])):
            raise EvidenceError(f"shared command executable identity is invalid: {name}")
    if command["executables"]["just"][0] != just_path:
        raise EvidenceError("shared command binding does not identify the authenticated just executable")


def validate_just_release_metadata(value: Any) -> dict[str, str]:
    if (not isinstance(value, dict) or not isinstance(value.get("tag_name"), str)
            or re.fullmatch(r"\d+\.\d+\.\d+", value["tag_name"]) is None
            or value.get("draft") is not False or value.get("prerelease") is not False
            or not isinstance(value.get("assets"), list)):
        raise EvidenceError("official just latest release metadata is invalid")
    tag = value["tag_name"]
    asset_name = f"just-{tag}-aarch64-apple-darwin.tar.gz"
    assets = [asset for asset in value["assets"] if isinstance(asset, dict) and asset.get("name") == asset_name]
    if len(assets) != 1:
        raise EvidenceError("official just latest Apple Silicon asset is missing or ambiguous")
    asset = assets[0]
    digest = asset.get("digest")
    size = asset.get("size")
    asset_url = f"https://github.com/{JUST_RELEASE_REPOSITORY}/releases/download/{tag}/{asset_name}"
    if (asset.get("browser_download_url") != asset_url
            or not isinstance(digest, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", digest) is None
            or type(size) is not int or size < 1 or size > MAX_JUST_ASSET_BYTES):
        raise EvidenceError("official just latest asset identity is invalid")
    return {"tag": tag, "asset": asset_name, "url": asset_url, "sha256": digest.removeprefix("sha256:")}


def parse_stable_manifest_versions(manifest: str) -> dict[str, str]:
    """Derive stable CLI versions from the official channel manifest."""
    result: dict[str, str] = {}
    for package in ("rustc", "cargo", "clippy-preview", "rustfmt-preview"):
        section = re.search(
            rf"(?ms)^\[pkg\.{package}\]\s*$\n(?P<body>.*?)(?=^\[|\Z)", manifest
        )
        version = re.search(r'^version\s*=\s*"([^"]+)"\s*$', section["body"], re.MULTILINE) if section else None
        if version is None:
            raise EvidenceError(f"official Rust stable manifest is missing {package} version")
        result[package.removesuffix("-preview")] = version.group(1)
    # Cargo内部版は0.xのため、Rustと同期するCLI版とCargo自身のrevisionを組み合わせる。
    rust_release = re.match(r"^(\d+\.\d+\.\d+)(?: \([^)]*\))?$", result["rustc"])
    cargo_commit = re.fullmatch(r"0\.\d+\.\d+ \(([0-9a-f]{9,40}) (\d{4}-\d{2}-\d{2})\)", result["cargo"])
    if rust_release is None or cargo_commit is None:
        raise EvidenceError("official Rust stable manifest has unsupported tool version metadata")
    result["cargo_cli"] = f"{rust_release.group(1)} ({cargo_commit.group(1)[:9]} {cargo_commit.group(2)})"
    rustc_revision = re.fullmatch(r"\d+\.\d+\.\d+ \(([0-9a-f]{9,40}) (\d{4}-\d{2}-\d{2})\)", result["rustc"])
    rustc_section = re.search(r"(?ms)^\[pkg\.rustc\]\s*$\n(?P<body>.*?)(?=^\[|\Z)", manifest)
    rustc_commit = re.search(r'^git_commit_hash\s*=\s*"([0-9a-f]{40})"\s*$', rustc_section["body"], re.MULTILINE) if rustc_section else None
    component_commits = []
    for package in ("clippy-preview", "rustfmt-preview"):
        section = re.search(rf"(?ms)^\[pkg\.{package}\]\s*$\n(?P<body>.*?)(?=^\[|\Z)", manifest)
        commit = re.search(r'^git_commit_hash\s*=\s*"([0-9a-f]{40})"\s*$', section["body"], re.MULTILINE) if section else None
        component_commits.append(commit.group(1) if commit else None)
    clippy_version = re.fullmatch(r"\d+\.\d+\.\d+", result["clippy"])
    rustfmt_version = re.fullmatch(r"\d+\.\d+\.\d+", result["rustfmt"])
    if (rustc_revision is None or rustc_commit is None or clippy_version is None or rustfmt_version is None
            or rustc_revision.group(1) != rustc_commit.group(1)[:9]
            or component_commits != [rustc_commit.group(1), rustc_commit.group(1)]):
        raise EvidenceError("official Rust stable manifest has unsupported component version metadata")
    revision = f"{rustc_commit.group(1)[:10]} {rustc_revision.group(2)}"
    result["clippy_cli"] = f"{result['clippy']} ({revision})"
    result["rustfmt_cli"] = f"{result['rustfmt']}-stable ({revision})"
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
    for package, version_key in (
        ("rustc", "rustc"), ("cargo", "cargo_cli"),
        ("clippy", "clippy_cli"), ("rustfmt", "rustfmt_cli"),
    ):
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
    stable_versions: dict[str, str], just_release_metadata: dict[str, str],
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
        "macos", "architecture", "rust_host", "rustc", "cargo", "clippy", "rustfmt",
        "java", "java_home", "java_vendor", "bun", "just", "just_path", "just_release",
        "just_asset", "just_asset_url", "just_asset_sha256", "just_binary_sha256", "command_binding", "brew", "graphviz"
    } or any(not isinstance(value, str) or not value.strip() for value in tools.values()):
        return False
    if not Path(tools["java_home"]).is_absolute():
        return False
    if (tools.get("architecture") != "arm64" or not is_macos_15(tools.get("macos"))
            or tools.get("rust_host") != SCOPE_PARAMETERS["CARGO_BUILD_TARGET"]):
        return False
    if (not java_is_21(tools.get("java")) or tools.get("java_vendor") != JAVA_VENDOR
            or tools.get("bun") != "1.4.2"):
        return False
    try:
        validate_stable_rust_tools(tools, stable_versions)
        validate_pinned_tools(tools)
    except EvidenceError:
        return False
    if (tools.get("just_release") != just_release_metadata.get("tag")
            or tools.get("just_asset") != just_release_metadata.get("asset")
            or tools.get("just_asset_url") != just_release_metadata.get("url")
            or tools.get("just_asset_sha256") != just_release_metadata.get("sha256")):
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
    repository, number, base_sha, head_sha, workflow_hash, now, owner_login, stable_versions, just_release_metadata = args
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
        comment, payload, repository, number, base_sha, head_sha, workflow_hash, now, owner_login,
        stable_versions, just_release_metadata,
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
        just_release_metadata = validate_just_release_metadata(
            fetch(f"repos/{JUST_RELEASE_REPOSITORY}/releases/latest")
        )
        first = accepted_comment(
            comments_first, repository, number, base_sha, head_sha, digest, timestamp, owner_login,
            stable_versions, just_release_metadata,
        )
        second = accepted_comment(
            comments_second, repository, number, base_sha, head_sha, digest, timestamp, owner_login,
            stable_versions, just_release_metadata,
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
    isolated_inputs_dir = Path(tempfile.mkdtemp(prefix="krr-local-macos-inputs-"))
    cargo_home = isolated_inputs_dir / "cargo-home"
    cargo_home.mkdir()
    cargo_target_dir = isolated_inputs_dir / "cargo-target"
    cargo_target_dir.mkdir()
    bun_cache_dir = isolated_inputs_dir / "bun-cache"
    bun_cache_dir.mkdir()
    logs: list[dict[str, Any]] = []
    smoke_output = str(logs_dir / "plantuml-sequence.svg")
    bootstrap_env, just_binding, binding, command_env = prepare_command_environment(
        token, isolated_inputs_dir, cargo_target_dir, cargo_home, bun_cache_dir,
    )

    def run_command(command_id: str, argv_template: tuple[str, ...]) -> None:
        assert_command_binding(binding, just_binding)
        argv = bind_command(
            tuple(part.format(plantuml_output=smoke_output) for part in argv_template), just_binding
        )
        start = time.monotonic()
        result = subprocess.run(argv, cwd=ROOT, env=command_env, text=True, capture_output=True)
        assert_command_binding(binding, just_binding)
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

    try:
        for command_id, argv_template in COMMANDS:
            if command_id in PREPARATION_COMMANDS:
                if command_id == "bun-install":
                    # ignoredな既存依存ファイルを検査対象へ持ち越さないよう、install前に再構築する。
                    clean_existing_node_modules()
                run_command(command_id, argv_template)
        stable_versions_before = fetch_stable_manifest_versions()
        versions_before = tool_versions(cargo_target_dir, cargo_home, bun_cache_dir, just_binding, binding)
        validate_pinned_tools(versions_before)
        validate_stable_rust_tools(versions_before, stable_versions_before)
        for command_id, argv_template in COMMANDS:
            if command_id not in PREPARATION_COMMANDS:
                run_command(command_id, argv_template)
        stable_versions_after = fetch_stable_manifest_versions()
        versions_after = tool_versions(cargo_target_dir, cargo_home, bun_cache_dir, just_binding, binding)
        validate_pinned_tools(versions_after)
        validate_stable_rust_tools(versions_after, stable_versions_after)
        if versions_before != versions_after or stable_versions_before != stable_versions_after:
            raise EvidenceError("tool versions changed during local checks")
    finally:
        shutil.rmtree(isolated_inputs_dir)
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
