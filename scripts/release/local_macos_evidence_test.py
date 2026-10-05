from __future__ import annotations

import copy
import importlib.util
import io
import json
import os
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import Mock, patch


MODULE_PATH = Path(__file__).with_name("local_macos_evidence.py")
SPEC = importlib.util.spec_from_file_location("local_macos_evidence", MODULE_PATH)
assert SPEC and SPEC.loader
EVIDENCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(EVIDENCE)


def collector_environment(cargo_target_dir: Path) -> dict[str, str]:
    return EVIDENCE.command_environment(
        cargo_target_dir,
        cargo_target_dir.parent / "evidence-cargo-home",
        cargo_target_dir.parent / "evidence-bun-cache",
    )


def collector_binding_patches():
    fake_just = EVIDENCE.JustBinding(
        Path("/trusted-just/just"), "just 1.40.0", "1.40.0", "just-1.40.0-aarch64-apple-darwin.tar.gz",
        "https://github.com/casey/just/releases/download/1.40.0/just-1.40.0-aarch64-apple-darwin.tar.gz", "a" * 64, "b" * 64,
    )
    fake_bun = fixture_bun_binding()
    return patch.multiple(
        EVIDENCE,
        download_trusted_just=Mock(return_value=fake_just),
        download_trusted_bun=Mock(return_value=fake_bun),
        command_binding=Mock(return_value=EVIDENCE.CommandBinding(
            "/bin", {}, {}, RUST_COMPONENT_DIGESTS, fixture_host_tool_proof(),
        )),
        fetch_official_rust_component_proof=Mock(return_value=fixture_rust_proof()),
        command_environment=Mock(return_value={"PATH": "/bin", "JAVA_HOME": "/java"}),
        assert_command_binding=Mock(),
        bind_command=Mock(side_effect=lambda argv, _binding: list(argv)),
    )


REPOSITORY = "owner/repository"
BASE = "a" * 40
HEAD = "b" * 40
NOW = 1_800_000_000
STABLE_RUST = {
    "rustc": "1.99.0 (b940084d7 2026-09-28)",
    "cargo": "0.100.0 (5f94df478 2026-08-27)",
    "cargo_cli": "1.99.0 (5f94df478 2026-08-27)",
    "clippy": "0.1.99",
    "clippy_cli": "0.1.99 (b940084d7e 2026-09-28)",
    "rustfmt": "1.10.0",
    "rustfmt_cli": "1.10.0-stable (b940084d7e 2026-09-28)",
}

BUN_BINARY = b"#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 1.4.2; else exit 0; fi\n"


def fixture_bun_archive(binary: bytes = BUN_BINARY) -> bytes:
    archive = io.BytesIO()
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_STORED) as output:
        directory = zipfile.ZipInfo("bun-darwin-aarch64/")
        directory.external_attr = (stat.S_IFDIR | 0o755) << 16 | 0x10
        output.writestr(directory, b"")
        executable = zipfile.ZipInfo("bun-darwin-aarch64/bun")
        executable.external_attr = (stat.S_IFREG | 0o755) << 16
        output.writestr(executable, binary)
    return archive.getvalue()


BUN_ARCHIVE = fixture_bun_archive()
BUN_ARCHIVE_SHA256 = EVIDENCE.sha256_bytes(BUN_ARCHIVE)
BUN_BINARY_SHA256 = EVIDENCE.sha256_bytes(BUN_BINARY)
BUN_ASSET_URL = (
    f"https://github.com/{EVIDENCE.BUN_RELEASE_REPOSITORY}/releases/download/"
    f"{EVIDENCE.BUN_RELEASE_TAG}/{EVIDENCE.BUN_ASSET_NAME}"
)
BUN_RELEASE = {
    "tag_name": EVIDENCE.BUN_RELEASE_TAG,
    "draft": False,
    "prerelease": False,
    "assets": [{
        "name": EVIDENCE.BUN_ASSET_NAME,
        "browser_download_url": BUN_ASSET_URL,
        "digest": f"sha256:{BUN_ARCHIVE_SHA256}",
        "size": len(BUN_ARCHIVE),
    }],
}
RUST_COMPONENT_DIGESTS = {
    name: f"{index:x}" * 64 for index, name in enumerate(EVIDENCE.RUST_COMPONENT_PACKAGES)
}
RUST_EXECUTABLE_DIGESTS = {
    name: f"{index + 1:x}" * 64 for index, name in enumerate(EVIDENCE.RUST_EXECUTABLE_PATHS)
}


def fixture_host_tool_proof():
    dot_path = "/opt/homebrew/Cellar/graphviz/12.0.0/bin/dot"
    return EVIDENCE.TOOL_PROVENANCE.HostToolProof(
        {
            dot_path: EVIDENCE.TOOL_PROVENANCE.InventoryEntry("file", 0o111, "c" * 64),
        },
        {
            "bin/java": EVIDENCE.TOOL_PROVENANCE.InventoryEntry("file", 0o111, "d" * 64),
        }, {
            "graphviz": [{
                "name": "graphviz", "version": "12.0.0", "bottle_tag": "arm64_sequoia",
                "url": "https://ghcr.io/v2/homebrew/core/graphviz/blobs/sha256:" + "a" * 64,
                "sha256": "a" * 64,
            }],
            "java": {
                "release": "jdk-21.0.12+7", "asset": "OpenJDK21U-jdk_aarch64_mac_hotspot_21.0.12_7.tar.gz",
                "url": "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.12%2B7/OpenJDK21U-jdk_aarch64_mac_hotspot_21.0.12_7.tar.gz",
                "sha256": "b" * 64,
            },
            "host_tcb": EVIDENCE.TOOL_PROVENANCE.HOST_TCB_POLICY,
        },
        dot_path,
    )


def fixture_rust_proof():
    return EVIDENCE.RustComponentProof({}, RUST_COMPONENT_DIGESTS, RUST_EXECUTABLE_DIGESTS)


def fixture_installed_rust_proof(rust_root: Path):
    files = {}
    executables = {}
    for name, relative in EVIDENCE.RUST_EXECUTABLE_PATHS.items():
        path = rust_root / relative
        identity = (stat.S_IMODE(path.stat().st_mode) & 0o111, EVIDENCE.sha256_bytes(path.read_bytes()))
        files[relative] = identity
        executables[name] = identity[1]
    standard_library = rust_root / "lib/rustlib/aarch64-apple-darwin/lib/libstd.rlib"
    files[standard_library.relative_to(rust_root).as_posix()] = (
        stat.S_IMODE(standard_library.stat().st_mode) & 0o111,
        EVIDENCE.sha256_bytes(standard_library.read_bytes()),
    )
    return EVIDENCE.RustComponentProof(files, RUST_COMPONENT_DIGESTS, executables)


def fixture_rust_archives():
    components = []
    archives = {}
    tool_files = {
        "rustc": {"bin/rustc": b"official-rustc"},
        "cargo": {"bin/cargo": b"official-cargo"},
        "clippy": {"bin/cargo-clippy": b"official-cargo-clippy", "bin/clippy-driver": b"official-clippy-driver"},
        "rustfmt": {"bin/rustfmt": b"official-rustfmt"},
        "rust-std": {"lib/rustlib/aarch64-apple-darwin/lib/libstd.rlib": b"official-std"},
    }
    for name, package_dir in EVIDENCE.RUST_COMPONENT_ARCHIVE_DIRS.items():
        top = f"{package_dir}-1.99.0-{EVIDENCE.RUST_HOST}"
        files = tool_files[name]
        manifest = "".join(f"file:{path}\n" for path in files)
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode="w:xz") as bundle:
            for directory in (top, f"{top}/{package_dir}"):
                member = tarfile.TarInfo(directory)
                member.type = tarfile.DIRTYPE
                bundle.addfile(member)
            metadata = tarfile.TarInfo(f"{top}/{package_dir}/manifest.in")
            metadata.size = len(manifest.encode())
            bundle.addfile(metadata, io.BytesIO(manifest.encode()))
            for path, data in files.items():
                member = tarfile.TarInfo(f"{top}/{package_dir}/{path}")
                member.mode = 0o755
                member.size = len(data)
                bundle.addfile(member, io.BytesIO(data))
        archive = output.getvalue()
        digest = EVIDENCE.sha256_bytes(archive)
        components.append(EVIDENCE.RustComponent(
            name, f"https://static.rust-lang.org/dist/2026-10-01/"
                 f"{EVIDENCE.RUST_COMPONENT_ARCHIVE_NAMES[name]}-1.99.0-{EVIDENCE.RUST_HOST}.tar.xz",
            digest,
        ))
        archives[name] = archive
    return tuple(components), archives


def fixture_bun_binding():
    return EVIDENCE.BunBinding(
        EVIDENCE.BUN_RELEASE_TAG, EVIDENCE.BUN_ASSET_NAME, BUN_ASSET_URL,
        BUN_ARCHIVE_SHA256, BUN_BINARY_SHA256,
    )


class FixtureDownloadResponse:
    def __init__(self, archive: bytes = BUN_ARCHIVE, url: str = BUN_ASSET_URL) -> None:
        self.archive = archive
        self.url = url

    def __enter__(self):
        return self

    def __exit__(self, _type, _value, _traceback):
        return False

    def geturl(self) -> str:
        return self.url

    def read(self, _size: int = -1) -> bytes:
        return self.archive if _size < 0 else self.archive[:_size]


def valid_comment() -> dict[str, object]:
    commands = [
        {
            "id": command_id,
            "argv": list(argv),
            "exit_code": 0,
            "duration_ms": 1,
            "log_sha256": "c" * 64,
        }
        for command_id, argv in EVIDENCE.COMMANDS
    ]
    payload = {
        "schema": EVIDENCE.SCHEMA,
        "repository": REPOSITORY,
        "pull_request": 7,
        "base_sha": BASE,
        "head_sha": HEAD,
        "workflow_sha256": "d" * 64,
        "scope_sha256": EVIDENCE.scope_digest("d" * 64),
        "completed_at": "2027-01-15T08:00:00Z",
        "platform": "macos-arm64",
        "tools": {
            "macos": "15.0", "architecture": "arm64", "rust_host": "aarch64-apple-darwin",
            "rustc": f"rustc {STABLE_RUST['rustc']}",
            "cargo": f"cargo {STABLE_RUST['cargo_cli']}", "java": 'openjdk version "21.0.12" 2025-01-21', "bun": "1.4.2",
            "clippy": f"clippy {STABLE_RUST['clippy_cli']}",
            "rustfmt": f"rustfmt {STABLE_RUST['rustfmt_cli']}",
            "java_home": "/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home",
            "java_vendor": "Eclipse Adoptium",
            "just": "just 1.40.0", "graphviz": "dot - graphviz version 12.0.0",
            "just_path": "/tmp/krr-local-macos-inputs-fixture/trusted-just/just",
            "just_release": "1.40.0", "just_asset": "just-1.40.0-aarch64-apple-darwin.tar.gz",
            "just_asset_url": "https://github.com/casey/just/releases/download/1.40.0/just-1.40.0-aarch64-apple-darwin.tar.gz",
            "just_asset_sha256": "a" * 64, "just_binary_sha256": "b" * 64,
            "bun_release": EVIDENCE.BUN_RELEASE_TAG,
            "bun_asset": EVIDENCE.BUN_ASSET_NAME,
            "bun_asset_url": BUN_ASSET_URL,
            "bun_asset_sha256": BUN_ARCHIVE_SHA256,
            "bun_binary_sha256": BUN_BINARY_SHA256,
            "command_binding": json.dumps({
                "path": os.pathsep.join((
                    "/tmp/krr-local-macos-inputs-fixture/trusted-commands",
                    *EVIDENCE.SYSTEM_COMMAND_PATH,
                )),
                "path_policy": EVIDENCE.COMMAND_PATH_POLICY,
                "executables": {
                    name: [
                        "/tmp/krr-local-macos-inputs-fixture/trusted-just/just" if name == "just"
                        else "/opt/homebrew/bin/bun" if name == "bun"
                        else fixture_host_tool_proof().dot_path if name == "dot"
                        else "/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home/bin/java" if name == "java"
                        else f"/trusted/{name}",
                        BUN_BINARY_SHA256 if name == "bun"
                        else fixture_host_tool_proof().graphviz_files[fixture_host_tool_proof().dot_path].value if name == "dot"
                        else fixture_host_tool_proof().java_files["bin/java"].value if name == "java"
                        else RUST_EXECUTABLE_DIGESTS.get(name, "c" * 64),
                    ]
                    for name in ("just", "cargo", "rustc", "cargo-clippy", "clippy-driver", "rustfmt", "bun", "dot", "bash", "python3", "java", "git", "source_git", "sw_vers", "uname")
                },
                "rust_components": RUST_COMPONENT_DIGESTS,
                "host_tool_provenance": EVIDENCE.TOOL_PROVENANCE.serialized_host_tool_proof(fixture_host_tool_proof()),
            }, sort_keys=True, separators=(",", ":")),
        },
        "commands": commands,
        "environment": EVIDENCE.SCOPE_PARAMETERS,
        "logs_sha256": "e" * 64,
    }
    return {
        "id": 41,
        "body": EVIDENCE.MARKER + "\n" + json.dumps(payload, separators=(",", ":")),
        "created_at": payload["completed_at"],
        "updated_at": payload["completed_at"],
        "author_association": "OWNER",
        "user": {"login": "owner"},
    }


def snapshots(
    comment: object, *, changed_second: bool = False, api_fail: bool = False,
    bun_release: object | None = None, bun_api_fail: bool = False,
):
    pr = {
        "number": 7,
        "state": "open",
        "base": {"sha": BASE, "ref": "master", "repo": {"full_name": REPOSITORY}},
        "head": {"sha": HEAD, "ref": "release/v1.2.3", "repo": {"full_name": REPOSITORY}},
    }
    repo = {"full_name": REPOSITORY, "owner": {"login": "owner"}}
    compare = {"merge_base_commit": {"sha": BASE}, "status": "ahead"}
    calls: dict[str, int] = {}

    def fetch(path: str):
        calls[path] = calls.get(path, 0) + 1
        if api_fail:
            raise RuntimeError("offline")
        if path == f"repos/{REPOSITORY}":
            return repo
        if path == f"repos/{REPOSITORY}/pulls/7":
            value = copy.deepcopy(pr)
            if changed_second and calls[path] == 2:
                value["head"]["sha"] = "f" * 40
            return value
        if path == f"repos/{REPOSITORY}/compare/{BASE}...{HEAD}":
            return compare
        if path == f"repos/{EVIDENCE.JUST_RELEASE_REPOSITORY}/releases/latest":
            return {
                "tag_name": "1.40.0", "draft": False, "prerelease": False,
                "assets": [{
                    "name": "just-1.40.0-aarch64-apple-darwin.tar.gz",
                    "browser_download_url": "https://github.com/casey/just/releases/download/1.40.0/just-1.40.0-aarch64-apple-darwin.tar.gz",
                    "digest": "sha256:" + "a" * 64, "size": 123,
                }],
            }
        if path == f"repos/{EVIDENCE.BUN_RELEASE_REPOSITORY}/releases/tags/{EVIDENCE.BUN_RELEASE_TAG}":
            if bun_api_fail:
                raise RuntimeError("Bun release metadata unavailable")
            return copy.deepcopy(BUN_RELEASE if bun_release is None else bun_release)
        if path.startswith(f"repos/{REPOSITORY}/issues/7/comments?"):
            if isinstance(comment, list):
                return copy.deepcopy(comment)
            return [copy.deepcopy(comment)] if comment is not None else []
        raise AssertionError(f"unexpected API path {path}")

    return fetch


class LocalMacosEvidenceTest(unittest.TestCase):
    def setUp(self) -> None:
        patcher = patch.object(EVIDENCE, "fetch_stable_manifest_versions", return_value=STABLE_RUST)
        patcher.start()
        self.addCleanup(patcher.stop)
        bun_asset_patcher = patch.object(
            EVIDENCE.request, "urlopen", return_value=FixtureDownloadResponse(),
        )
        bun_asset_patcher.start()
        self.addCleanup(bun_asset_patcher.stop)
        rust_proof_patcher = patch.object(
            EVIDENCE, "fetch_official_rust_component_proof", return_value=fixture_rust_proof(),
        )
        rust_proof_patcher.start()
        self.addCleanup(rust_proof_patcher.stop)
        host_tool_patcher = patch.object(
            EVIDENCE, "fetch_official_host_tool_proof", return_value=fixture_host_tool_proof(),
        )
        host_tool_patcher.start()
        self.addCleanup(host_tool_patcher.stop)
        verify_host_tools_patcher = patch.object(EVIDENCE, "verify_installed_host_tools")
        verify_host_tools_patcher.start()
        self.addCleanup(verify_host_tools_patcher.stop)

    def test_graphviz_workflow_step_is_satisfied_by_authenticated_preinstalled_tree(self) -> None:
        self.assertTrue(EVIDENCE.workflow_scope_supported())
        self.assertEqual(
            EVIDENCE.MAC_WORKFLOW_STEP_COMMANDS[
                "Install Graphviz for PlantUML on macOS"
            ],
            ("graphviz-install",),
        )
        self.assertIn(
            ("graphviz-install", ("brew", "install", "graphviz")), EVIDENCE.COMMANDS,
        )

    def test_prepare_binds_homebrew_bun_to_official_binary_sha(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            isolated = Path(temporary) / "krr-local-macos-inputs-fixture"
            isolated.mkdir()
            cargo_home = isolated / "cargo-home"
            cargo_home.mkdir()
            cargo_target = isolated / "cargo-target"
            cargo_target.mkdir()
            bun_cache = isolated / "bun-cache"
            bun_cache.mkdir()
            just = EVIDENCE.JustBinding(
                Path("/trusted-just/just"), "just 1.40.0", "1.40.0",
                "just-1.40.0-aarch64-apple-darwin.tar.gz",
                "https://github.com/casey/just/releases/download/1.40.0/just-1.40.0-aarch64-apple-darwin.tar.gz",
                "a" * 64, "b" * 64,
            )
            command_binding = EVIDENCE.CommandBinding(
                "/bin", {}, {}, RUST_COMPONENT_DIGESTS, fixture_host_tool_proof(),
            )
            with (
                patch.object(
                    EVIDENCE, "command_environment",
                    side_effect=[{"PATH": "/bin"}, {"PATH": "/bin"}],
                ),
                patch.object(EVIDENCE, "download_trusted_just", return_value=just),
                patch.object(
                    EVIDENCE, "download_trusted_bun", return_value=fixture_bun_binding(),
                ),
                patch.object(
                    EVIDENCE, "command_binding", return_value=command_binding,
                ) as command_binding_mock,
                patch.object(
                    EVIDENCE, "macos_product_version", return_value="15.0",
                ) as macos_version_mock,
                patch.object(
                    EVIDENCE.subprocess, "run",
                    side_effect=AssertionError("preparation must not invoke host subprocesses"),
                ) as subprocess_run_mock,
            ):
                prepared = EVIDENCE.prepare_command_environment(
                    "token", isolated, cargo_target, cargo_home, bun_cache,
                )

            macos_version_mock.assert_called_once_with()
            subprocess_run_mock.assert_not_called()
            self.assertEqual(prepared, (
                {"PATH": "/bin"}, just, fixture_bun_binding(), command_binding, {"PATH": "/bin"},
            ))
            command_binding_mock.assert_called_once_with(
                {"PATH": "/bin"}, just, BUN_BINARY_SHA256, fixture_rust_proof(),
                fixture_host_tool_proof(),
            )

    def test_accepts_current_owner_proof_bound_to_base_head_workflow_and_full_scope(self) -> None:
        self.assertTrue(EVIDENCE.verify_from_api(
            REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=snapshots(valid_comment()), now=NOW, expected_workflow_digest="d" * 64,
        ))

    def test_gh_token_pins_public_github_when_gh_host_targets_enterprise(self) -> None:
        result = subprocess.CompletedProcess(
            ["gh", "auth", "token", "--hostname", "github.com"], 0, "public-token\n", "",
        )
        with patch.dict(EVIDENCE.os.environ, {"GH_HOST": "enterprise.example"}), patch.object(
            EVIDENCE.subprocess, "run", return_value=result,
        ) as run:
            token = EVIDENCE.gh_token()

        self.assertEqual(token, "public-token")
        self.assertEqual(run.call_args.args[0], ["gh", "auth", "token", "--hostname", "github.com"])

    def test_attestation_publication_pins_github_api_host_when_gh_host_targets_enterprise(self) -> None:
        result = subprocess.CompletedProcess(["gh", "api"], 0, '{"id": 1}', "")
        body_path = Path("/tmp/local-macos-evidence-body.json")
        with patch.dict(EVIDENCE.os.environ, {"GH_HOST": "enterprise.example"}), patch.object(
            EVIDENCE.subprocess, "run", return_value=result,
        ) as run:
            posted = EVIDENCE.publish_attestation_comment(REPOSITORY, 7, body_path)

        self.assertEqual(posted.stdout, '{"id": 1}')
        self.assertEqual(run.call_args.args[0], [
            "gh", "api", "--hostname", "github.com", f"repos/{REPOSITORY}/issues/7/comments",
            "--method", "POST", "--input", str(body_path),
        ])

    def test_rejects_missing_malformed_duplicate_and_non_owner_comments(self) -> None:
        good = valid_comment()
        malformed = {**good, "body": EVIDENCE.MARKER + "\n{"}
        duplicate = [good, {**good, "id": 42}]
        outsider = {**good, "user": {"login": "contributor"}}
        duplicate_keys = {**good, "body": EVIDENCE.MARKER + '\n{"schema":1,"schema":1}'}
        for candidate in (None, malformed, duplicate_keys, duplicate, outsider):
            with self.subTest(candidate=candidate):
                self.assertFalse(EVIDENCE.verify_from_api(
                    REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                    fetch=snapshots(candidate), now=NOW, expected_workflow_digest="d" * 64,
                ))

    def test_rejects_head_base_workflow_scope_and_command_mismatches(self) -> None:
        good = valid_comment()
        payload = json.loads(good["body"].split("\n", 1)[1])
        mutations = (
            ({"base_sha": "f" * 40}, "d" * 64),
            ({"head_sha": "f" * 40}, "d" * 64),
            ({"scope_sha256": "f" * 64}, "d" * 64),
            ({"commands": payload["commands"][:-1]}, "d" * 64),
            ({"commands": [{**payload["commands"][0], "exit_code": 1}, *payload["commands"][1:]]}, "d" * 64),
            ({"commands": [{**payload["commands"][0], "duration_ms": -1}, *payload["commands"][1:]]}, "d" * 64),
            ({"commands": [{**payload["commands"][0], "log_sha256": ""}, *payload["commands"][1:]]}, "d" * 64),
        )
        for mutation, workflow in mutations:
            changed = {**payload, **mutation}
            comment = {**good, "body": EVIDENCE.MARKER + "\n" + json.dumps(changed)}
            with self.subTest(mutation=mutation):
                self.assertFalse(EVIDENCE.verify_from_api(
                    REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                    fetch=snapshots(comment), now=NOW, expected_workflow_digest=workflow,
                ))
        self.assertFalse(EVIDENCE.verify_from_api(
            REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=snapshots(good), now=NOW, expected_workflow_digest="f" * 64,
        ))

    def test_rejects_legacy_or_broad_command_path_receipts(self) -> None:
        good = valid_comment()
        payload = json.loads(good["body"].split("\n", 1)[1])
        command = json.loads(payload["tools"]["command_binding"])
        for changed_binding in (
            {key: value for key, value in command.items() if key != "path_policy"},
            {**command, "path_policy": "legacy-broad-install-directories"},
            {**command, "path": command["path"].replace("/usr/bin", "/opt/homebrew/bin:/usr/bin")},
            {**command, "path": command["path"] + ":/opt/homebrew/bin"},
        ):
            tools = {**payload["tools"], "command_binding": json.dumps(changed_binding)}
            changed_payload = {**payload, "tools": tools}
            comment = {**good, "body": EVIDENCE.MARKER + "\n" + json.dumps(changed_payload)}
            with self.subTest(binding=changed_binding):
                self.assertFalse(EVIDENCE.verify_from_api(
                    REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                    fetch=snapshots(comment), now=NOW, expected_workflow_digest="d" * 64,
                ))

    def test_old_policy_and_missing_candidates_skip_native_archive_fetch(self) -> None:
        good = valid_comment()
        payload = json.loads(good["body"].split("\n", 1)[1])
        command = json.loads(payload["tools"]["command_binding"])
        command["path_policy"] = "isolated-bound-symlinks-plus-os-system-directories-v1"
        old_policy = {**good, "body": EVIDENCE.MARKER + "\n" + json.dumps({
            **payload, "tools": {**payload["tools"], "command_binding": json.dumps(command)},
        })}
        for comment in (None, old_policy):
            with patch.object(EVIDENCE, "fetch_official_host_tool_proof") as fetch_host_tools:
                self.assertFalse(EVIDENCE.verify_from_api(
                    REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                    fetch=snapshots(comment), now=NOW, expected_workflow_digest="d" * 64,
                ))
                fetch_host_tools.assert_not_called()

    def test_rejects_java_vendor_other_than_temurin_and_java_other_than_21(self) -> None:
        good = valid_comment()
        payload = json.loads(good["body"].split("\n", 1)[1])
        for java, bun in (
            ('openjdk version "17.0.13" 2024-10-15', "1.4.2"),
            ('openjdk version "22.0.1" 2024-10-15', "1.4.2"),
            ("Java runtime unknown", "1.4.2"),
            ('openjdk version "21.0.12" 2025-01-21', "1.4.1"),
        ):
            changed = {**payload, "tools": {**payload["tools"], "java": java, "bun": bun}}
            comment = {**good, "body": EVIDENCE.MARKER + "\n" + json.dumps(changed)}
            with self.subTest(java=java, bun=bun):
                self.assertFalse(EVIDENCE.verify_from_api(
                    REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                    fetch=snapshots(comment), now=NOW, expected_workflow_digest="d" * 64,
                ))

        for vendor in ("Oracle Corporation", "Azul Systems, Inc.", "", None):
            changed = {**payload, "tools": {**payload["tools"], "java_vendor": vendor}}
            comment = {**good, "body": EVIDENCE.MARKER + "\n" + json.dumps(changed)}
            with self.subTest(vendor=vendor):
                self.assertFalse(EVIDENCE.verify_from_api(
                    REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                    fetch=snapshots(comment), now=NOW, expected_workflow_digest="d" * 64,
                ))

        valid_tools = json.loads(valid_comment()["body"].split("\n", 1)[1])["tools"]
        command = json.loads(valid_tools["command_binding"])
        command["executables"]["java"][0] = "/jdk/21/bin/java"
        EVIDENCE.validate_pinned_tools({**valid_tools,
            "java": 'java version "21.0.12"', "java_home": "/jdk/21",
            "java_vendor": "Eclipse Adoptium", "bun": "1.4.2",
            "command_binding": json.dumps(command),
        })
        for java, bun in (
            ('java version "17.0.13"', "1.4.2"),
            ('java version "22.0.1"', "1.4.2"),
            ("unparseable", "1.4.2"),
            ('java version "21.0.12"', "1.4.1"),
        ):
            with self.subTest(collection_java=java, collection_bun=bun):
                with self.assertRaises(EVIDENCE.EvidenceError):
                    EVIDENCE.validate_pinned_tools({
                        "java": java, "java_home": "/jdk/21", "java_vendor": "Eclipse Adoptium", "bun": bun,
                    })
        for vendor in ("Oracle Corporation", "", None):
            with self.subTest(collection_vendor=vendor):
                with self.assertRaises(EVIDENCE.EvidenceError):
                    EVIDENCE.validate_pinned_tools({
                        "java": 'java version "21.0.12"', "java_home": "/jdk/21",
                        "java_vendor": vendor, "bun": "1.4.2",
                    })

    def test_java_home_is_derived_from_and_matches_the_java_21_runtime(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory) / "jdk-21"
            (home / "bin").mkdir(parents=True)
            executable = home / "bin/java"
            executable.write_text("fixture", encoding="utf-8")
            libjvm = home / "lib/server/libjvm.dylib"
            libjvm.parent.mkdir(parents=True)
            libjvm.write_text("fixture", encoding="utf-8")
            settings = f'    java.home = {home}\n    java.vendor = Eclipse Adoptium\nopenjdk version "21.0.12" 2025-01-21\n'
            with patch.object(EVIDENCE.subprocess, "run", side_effect=(
                subprocess.CompletedProcess([], 0, "", settings),
                subprocess.CompletedProcess([], 0, "", settings),
            )) as run:
                detected = EVIDENCE.detect_java_home({"PATH": "/safe/bin"})
            self.assertEqual(detected, str(home.resolve()))
            self.assertEqual(run.call_args_list[1].args[0], (str(executable.resolve()), "-XshowSettings:properties", "-version"))
            for bad_output in (
                f"java.home = {home}\njava.vendor = Eclipse Adoptium\nopenjdk version \"17.0.13\"",
                'java.home = /jdk/21\njava.vendor = Oracle Corporation\nopenjdk version "21.0.12"',
                f"java.home = {home}\njava.vendor = Eclipse Adoptium\njava.vendor = Oracle Corporation\nopenjdk version \"21.0.12\"",
                'openjdk version "21.0.12"',
                "java.home = relative/home\njava.vendor = Eclipse Adoptium\nopenjdk version \"21.0.12\"",
            ):
                with self.subTest(output=bad_output):
                    with self.assertRaises(EVIDENCE.EvidenceError):
                        EVIDENCE.java_home_from_settings(bad_output)
            with patch.object(EVIDENCE.subprocess, "run", side_effect=(
                subprocess.CompletedProcess([], 0, "", settings),
                subprocess.CompletedProcess([], 0, "", 'java.home = /jdk/21\njava.vendor = Eclipse Adoptium\nopenjdk version "17.0.13"\n'),
            )):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "JAVA_HOME does not identify Java 21"):
                    EVIDENCE.detect_java_home({"PATH": "/safe/bin"})
            libjvm.unlink()
            with patch.object(EVIDENCE.subprocess, "run", side_effect=(
                subprocess.CompletedProcess([], 0, "", settings),
            )):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "no PlantUML JVM library"):
                    EVIDENCE.detect_java_home({"PATH": "/safe/bin"})

    def test_only_macos_15_and_arm64_rust_host_are_accepted(self) -> None:
        good = valid_comment()
        payload = json.loads(good["body"].split("\n", 1)[1])
        for macos, rust_host in (
            ("13.7.8", "aarch64-apple-darwin"),
            ("14.7.8", "aarch64-apple-darwin"),
            ("27.0", "aarch64-apple-darwin"),
            ("15.foo", "aarch64-apple-darwin"),
            ("15.7junk", "aarch64-apple-darwin"),
            ("15.0", "x86_64-apple-darwin"),
            (None, "aarch64-apple-darwin"),
            ("15.0", None),
        ):
            changed = {
                **payload,
                "tools": {**payload["tools"], "macos": macos, "rust_host": rust_host},
            }
            comment = {**good, "body": EVIDENCE.MARKER + "\n" + json.dumps(changed)}
            with self.subTest(macos=macos, rust_host=rust_host):
                self.assertFalse(EVIDENCE.verify_from_api(
                    REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                    fetch=snapshots(comment), now=NOW, expected_workflow_digest="d" * 64,
                ))

    def test_collector_environment_forces_native_target_over_environment_and_config(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            config_dir = Path(directory) / ".cargo"
            config_dir.mkdir()
            (config_dir / "config.toml").write_text(
                'paths = []\n'
                '[build]\njobs = 2\ntarget-dir = "target-cache"\n'
                'target = "x86_64-apple-darwin"\nrustc = "alternate-rustc"\n'
                'rustc-wrapper = "compiler-wrapper"\nrustc-workspace-wrapper = "workspace-wrapper"\n'
                '[target.aarch64-apple-darwin]\nrunner = "/usr/bin/true"\n'
                'rustflags = ["--cfg", "local_config"]\n',
                encoding="utf-8",
            )
            with patch.object(EVIDENCE, "ROOT", Path(directory)), patch.object(
                EVIDENCE, "detect_java_home", return_value="/validated/JDK21",
            ), patch.dict(EVIDENCE.os.environ, {
                "CARGO": "true",
                "CARGO_BUILD_TARGET": "x86_64-apple-darwin",
                "RTK": "true",
                "RUSTFLAGS": "--target=x86_64-apple-darwin",
                "CARGO_ENCODED_RUSTFLAGS": "--target=x86_64-apple-darwin",
                "CARGO_BUILD_RUSTFLAGS": "--target=x86_64-apple-darwin",
                "CARGO_TARGET_DIR": "/tmp/cargo-target-cache",
                "BUN_INSTALL_CACHE_DIR": "/tmp/bun-cache",
                "HOME": directory,
                "CARGO_HOME": str(Path(directory) / "cargo-home"),
            }, clear=True):
                cargo_target_dir = Path(directory) / "isolated-cargo-target"
                environment = collector_environment(cargo_target_dir)
            self.assertEqual(environment["CARGO"], "cargo")
            self.assertEqual(environment["RTK"], "")
            self.assertEqual(environment["CARGO_BUILD_TARGET"], "aarch64-apple-darwin")
            self.assertEqual(environment["RUSTFLAGS"], "-D warnings")
            self.assertEqual(environment["CARGO_ENCODED_RUSTFLAGS"], "-D\x1fwarnings")
            self.assertEqual(environment["CARGO_BUILD_RUSTFLAGS"], "-D warnings")
            self.assertEqual(environment["RUSTC"], "rustc")
            self.assertEqual(environment["CARGO_BUILD_RUSTC"], "rustc")
            self.assertEqual(environment["RUSTC_WRAPPER"], "")
            self.assertEqual(environment["CARGO_BUILD_RUSTC_WRAPPER"], "")
            self.assertEqual(environment["RUSTC_WORKSPACE_WRAPPER"], "")
            self.assertEqual(environment["CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"], "")
            self.assertEqual(environment["CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER"], "/usr/bin/env")
            self.assertEqual(environment["CARGO_TARGET_DIR"], str(cargo_target_dir))
            self.assertEqual(environment["CARGO_HOME"], str(Path(directory) / "evidence-cargo-home"))
            self.assertEqual(environment["BUN_INSTALL_CACHE_DIR"], str(Path(directory) / "evidence-bun-cache"))
            self.assertIn('target = "x86_64-apple-darwin"', (config_dir / "config.toml").read_text())
            self.assertIn('runner = "/usr/bin/true"', (config_dir / "config.toml").read_text())
            with patch.dict(EVIDENCE.os.environ, {"CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER": "cross-linker"}, clear=True):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "custom Rust compiler or target linker"):
                    collector_environment(Path(directory) / "isolated-cargo-target")
            with patch.dict(EVIDENCE.os.environ, {"CARGO_BUILD_RUSTC_WRAPPER": "compiler-wrapper"}, clear=True):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "custom Rust compiler or target linker"):
                    collector_environment(Path(directory) / "isolated-cargo-target")

    def test_command_environment_drops_inherited_runtime_overrides_for_child(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            hostile = {
                "JAVA_HOME": "/wrong/jdk", "JAVA_TOOL_OPTIONS": "-Dwrong=true",
                "JDK_JAVA_OPTIONS": "-Dwrong=true", "_JAVA_OPTIONS": "-Dwrong=true",
                "KRR_PLANTUML_JVM": "/wrong/java", "KDR_PLANTUML_JVM": "/wrong/java",
                "KRR_PLANTUML_JAR": "/wrong/plantuml.jar", "PLANTUML_JAR": "/wrong/plantuml.jar",
                "KRR_PLANTUML_CACHE_DIR": "/wrong/cache", "KDR_PLANTUML_CACHE_DIR": "/wrong/cache",
                "MERMAID_JS": "/wrong/mermaid.js", "DRAWIO_JS": "/wrong/drawio.js",
                "MATHJAX_JS": "/wrong/mathjax.js", "DYLD_LIBRARY_PATH": "/wrong",
                "DYLD_INSERT_LIBRARIES": "/wrong", "LD_PRELOAD": "/wrong",
                "LD_LIBRARY_PATH": "/wrong", "BASH_ENV": "/wrong/bashrc", "ENV": "/wrong/env",
                "PYTHONPATH": "/wrong/python", "BUN_INSTALL": "/wrong/bun",
                "BUN_INSTALL_CACHE_DIR": "/wrong/bun-cache",
                "NODE_OPTIONS": "--require=/wrong.js", "JOBS": "99", "KRR_BIN": "/wrong/krr",
                "XDG_CACHE_HOME": "/wrong/cache", "CARGO": "true", "RTK": "true",
                "CARGO_ALIAS_CLIPPY": "--help", "CARGO_ALIAS_FMT": "--help",
                "RUSTFLAGS": "--cfg wrong",
            }
            safe = {
                "PATH": EVIDENCE.os.environ.get("PATH", "/usr/bin"),
                "HOME": directory, "TMPDIR": directory, "CARGO_HOME": str(Path(directory) / "cargo-home"),
                "RUSTUP_HOME": str(Path(directory) / "rustup"), "LANG": "C", "TERM": "dumb",
                **hostile,
            }
            with patch.object(EVIDENCE, "ROOT", Path(directory)), patch.object(
                EVIDENCE, "detect_java_home", return_value="/validated/JDK21",
            ), patch.dict(EVIDENCE.os.environ, safe, clear=True):
                environment = collector_environment(Path(directory) / "isolated-cargo-target")
            probe = (
                "import os,json; print(json.dumps({k:v for k,v in os.environ.items() "
                "if k in ('JAVA_HOME','JAVA_TOOL_OPTIONS','JDK_JAVA_OPTIONS','_JAVA_OPTIONS',"
                "'KRR_PLANTUML_JVM','KDR_PLANTUML_JVM','MERMAID_JS','DRAWIO_JS','MATHJAX_JS',"
                "'DYLD_LIBRARY_PATH','LD_PRELOAD','BASH_ENV','ENV','PYTHONPATH','NODE_OPTIONS',"
                "'CARGO','RTK','RUSTFLAGS','CARGO_BUILD_TARGET','CARGO_TARGET_DIR','CARGO_HOME',"
                "'BUN_INSTALL_CACHE_DIR')}))"
            )
            child = subprocess.run(
                [EVIDENCE.sys.executable, "-c", probe], env=environment,
                text=True, capture_output=True, check=True,
            )
            observed = json.loads(child.stdout)
            forced = {
                "CARGO": "cargo", "RTK": "", "RUSTFLAGS": "-D warnings",
                "CARGO_TARGET_DIR": str(Path(directory) / "isolated-cargo-target"),
                "CARGO_HOME": str(Path(directory) / "evidence-cargo-home"),
                "BUN_INSTALL_CACHE_DIR": str(Path(directory) / "evidence-bun-cache"),
            }
            for name in hostile.keys() - forced.keys():
                self.assertNotIn(name, environment, name)
                self.assertNotIn(name, observed, name)
            for name, value in forced.items():
                self.assertEqual(environment[name], value)
                self.assertEqual(observed[name], value)
            self.assertNotIn("JAVA_HOME", environment)
            self.assertNotIn("JAVA_HOME", observed)
            self.assertEqual(observed["CARGO_BUILD_TARGET"], "aarch64-apple-darwin")
            self.assertEqual(environment["CARGO_HOME"], str(Path(directory) / "evidence-cargo-home"))

    @unittest.skipUnless(os.name == "posix", "POSIX test executable fixture is required")
    def test_fresh_cargo_target_rebuilds_instead_of_reusing_tampered_test_binary(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = base / "repository"
            (root / "src").mkdir(parents=True)
            (root / "Cargo.toml").write_text(
                '[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2021"\n',
                encoding="utf-8",
            )
            (root / "src/lib.rs").write_text(
                '#[test]\nfn actual_failure() { assert!(false); }\n', encoding="utf-8",
            )
            old_target = root / "target"
            environment = {
                "HOME": str(base), "CARGO_HOME": str(base / "cargo-home"),
                "RUSTUP_HOME": EVIDENCE.os.environ.get("RUSTUP_HOME", str(Path(EVIDENCE.os.environ["HOME"]) / ".rustup")),
                "PATH": EVIDENCE.os.environ["PATH"],
            }
            rust_verbose = subprocess.run(
                ("rustc", "-vV"), env={**os.environ, **environment},
                text=True, capture_output=True, check=True,
            ).stdout
            rust_host = next(line.removeprefix("host: ") for line in rust_verbose.splitlines() if line.startswith("host: "))
            with patch.object(EVIDENCE, "ROOT", root), patch.object(
                EVIDENCE, "detect_java_home", return_value="/validated/JDK21",
            ), patch.dict(EVIDENCE.SCOPE_PARAMETERS, {"CARGO_BUILD_TARGET": rust_host}), patch.dict(
                EVIDENCE.os.environ, environment, clear=True,
            ):
                reused_environment = collector_environment(old_target)
                reused_environment["PATH"] = environment["PATH"]
                build = subprocess.run(
                    ("cargo", "test", "--no-run", "--lib", "--manifest-path", str(root / "Cargo.toml")),
                    cwd=root, env=reused_environment, text=True, capture_output=True,
                )
                self.assertEqual(build.returncode, 0, build.stdout + build.stderr)
                test_binaries = [
                    path for path in old_target.rglob("fixture-*")
                    if path.name.startswith("fixture-") and path.is_file() and path.suffix == ""
                ]
                self.assertEqual(len(test_binaries), 1)
                shutil.copyfile("/usr/bin/true", test_binaries[0])
                test_binaries[0].chmod(0o755)
                reused = subprocess.run(
                    ("cargo", "test", "--lib", "--manifest-path", str(root / "Cargo.toml")),
                    cwd=root, env=reused_environment, text=True, capture_output=True,
                )
                self.assertEqual(reused.returncode, 0, reused.stdout + reused.stderr)
                self.assertIn("unittests src/lib.rs", reused.stdout + reused.stderr)
                self.assertNotIn("actual_failure", reused.stdout + reused.stderr)

                isolated_target = base / "fresh-cargo-target"
                isolated_environment = collector_environment(isolated_target)
                isolated_environment["PATH"] = environment["PATH"]
                self.assertFalse(isolated_target.exists())
                isolated = subprocess.run(
                    ("cargo", "test", "--lib", "--manifest-path", str(root / "Cargo.toml")),
                    cwd=root, env=isolated_environment, text=True, capture_output=True,
                )
                self.assertNotEqual(isolated.returncode, 0)
                self.assertIn("actual_failure ... FAILED", isolated.stdout + isolated.stderr)

                passing_root = base / "passing"
                (passing_root / "src").mkdir(parents=True)
                (passing_root / "Cargo.toml").write_text(
                    '[package]\nname = "passing"\nversion = "0.1.0"\nedition = "2021"\n',
                    encoding="utf-8",
                )
                (passing_root / "src/lib.rs").write_text(
                    '#[test]\nfn actual_success() { assert!(true); }\n', encoding="utf-8",
                )
                positive_environment = collector_environment(base / "positive-cargo-target")
                positive_environment["PATH"] = environment["PATH"]
                positive = subprocess.run(
                    ("cargo", "test", "--lib", "--manifest-path", str(passing_root / "Cargo.toml")),
                    cwd=passing_root, env=positive_environment, text=True, capture_output=True,
                )
                self.assertEqual(positive.returncode, 0, positive.stdout + positive.stderr)
                self.assertIn("actual_success ... ok", positive.stdout + positive.stderr)

    def test_cargo_env_config_injection_rejects_local_proof(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config_dir = root / ".cargo"
            config_dir.mkdir()
            (config_dir / "config.toml").write_text(
                '[env]\nJAVA_HOME = { value = "/wrong/jdk", force = true }\n'
                'KRR_PLANTUML_JVM = { value = "/wrong/java", force = true }\n',
                encoding="utf-8",
            )
            with patch.object(EVIDENCE, "ROOT", root), patch.dict(EVIDENCE.os.environ, {
                "HOME": directory, "CARGO_HOME": str(root / "cargo-home"), "PATH": "/usr/bin",
            }, clear=True):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, r"Cargo config \[env\]"):
                    collector_environment(Path(directory) / "isolated-cargo-target")

    def test_relative_cargo_home_config_is_resolved_from_repository_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "repository"
            cargo_home = root / "relative-cargo-home"
            cargo_home.mkdir(parents=True)
            (cargo_home / "config.toml").write_text(
                '[env]\nJAVA_HOME = { value = "/wrong/jdk", force = true }\n',
                encoding="utf-8",
            )
            with patch.object(EVIDENCE, "ROOT", root), patch.dict(EVIDENCE.os.environ, {
                "HOME": directory, "CARGO_HOME": "relative-cargo-home", "PATH": "/usr/bin",
            }, clear=True):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, r"Cargo config \[env\]"):
                    collector_environment(Path(directory) / "isolated-cargo-target")

    def test_cargo_target_linker_config_rejects_triple_cfg_ancestor_and_cargo_home(self) -> None:
        cases = (
            ("repo", ".cargo/config.toml", '[target.aarch64-apple-darwin]\nlinker = ""\n'),
            ("ancestor", "../.cargo/config", "[target.'cfg(target_arch = \"aarch64\")']\nlinker = \"custom-linker\"\n"),
            ("cargo_home", "config.toml", '[target.aarch64-apple-darwin]\nlinker = "custom-linker"\n'),
            ("relative_cargo_home", "config", "[target.'cfg(target_os = \"macos\")']\nlinker = \"custom-linker\"\n"),
        )
        for location, relative_path, content in cases:
            with self.subTest(location=location, path=relative_path), tempfile.TemporaryDirectory() as directory:
                base = Path(directory)
                root = base / "repository"
                if location == "repo":
                    config_path = root / relative_path
                elif location == "ancestor":
                    config_path = base / ".cargo/config"
                elif location == "cargo_home":
                    config_path = root / "cargo-home" / relative_path
                else:
                    config_path = root / "relative-cargo-home" / relative_path
                config_path.parent.mkdir(parents=True, exist_ok=True)
                config_path.write_text(content, encoding="utf-8")
                cargo_home = {
                    "repo": str(base / "cargo-home"),
                    "ancestor": str(base / "cargo-home"),
                    "cargo_home": str(root / "cargo-home"),
                    "relative_cargo_home": "relative-cargo-home",
                }[location]
                with patch.object(EVIDENCE, "ROOT", root), patch.dict(EVIDENCE.os.environ, {
                    "HOME": str(base), "CARGO_HOME": cargo_home, "PATH": "/usr/bin",
                }, clear=True):
                    with self.assertRaisesRegex(EVIDENCE.EvidenceError, r"Cargo config target linker"):
                        collector_environment(Path(directory) / "isolated-cargo-target")

    def test_cargo_config_paths_include_profiles_and_links_overrides_reject(self) -> None:
        cases = (
            ("repo", "config.toml", 'paths = ["/tmp/modified-serde"]\n', r"Cargo config paths overrides"),
            ("ancestor", "config", "include = []\n", r"Cargo config includes"),
            ("cargo_home", "config.toml", "[profile.test]\ndebug-assertions = false\n", r"Cargo config profile overrides"),
            ("repo", "config.toml", '[build]\nwarnings = "allow"\n', r"Cargo config build.warnings override"),
            (
                "absolute_cargo_home", "config.toml",
                '[resolver]\nlockfile-path = "/tmp/other/Cargo.lock"\n',
                r"Cargo config resolver\.lockfile-path",
            ),
            (
                "relative_cargo_home", "config",
                "[target.aarch64-apple-darwin.ring_core_0_17_14_]\n"
                'rustc-link-lib = ["static=ring_core_0_17_14_"]\n',
                r"Cargo config target linker or links override",
            ),
            ("repo", "config.toml", '[alias]\nclippy = "--help"\n', r"Cargo config aliases for clippy/fmt"),
            ("cargo_home", "config", '[alias]\nfmt = "--help"\n', r"Cargo config aliases for clippy/fmt"),
        )
        for location, filename, content, expected_error in cases:
            with self.subTest(location=location), tempfile.TemporaryDirectory() as directory:
                base = Path(directory)
                root = base / "repository"
                config_parent = {
                    "repo": root / ".cargo",
                    "ancestor": base / ".cargo",
                    "cargo_home": root / "absolute-cargo-home",
                    "absolute_cargo_home": root / "other-absolute-cargo-home",
                    "relative_cargo_home": root / "relative-cargo-home",
                }[location]
                config_parent.mkdir(parents=True)
                (config_parent / filename).write_text(content, encoding="utf-8")
                cargo_home = {
                    "repo": str(base / "unused-cargo-home"),
                    "ancestor": str(base / "unused-cargo-home"),
                    "cargo_home": str(root / "absolute-cargo-home"),
                    "absolute_cargo_home": str(root / "other-absolute-cargo-home"),
                    "relative_cargo_home": "relative-cargo-home",
                }[location]
                with patch.object(EVIDENCE, "ROOT", root), patch.dict(EVIDENCE.os.environ, {
                    "HOME": str(base), "CARGO_HOME": cargo_home, "PATH": "/usr/bin",
                }, clear=True):
                    with self.assertRaisesRegex(EVIDENCE.EvidenceError, expected_error):
                        collector_environment(Path(directory) / "isolated-cargo-target")

    def test_real_cargo_alias_redirect_is_rejected_before_evidence_collection(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = base / "repository"
            cargo_home = base / "cargo-home"
            root.mkdir()
            cargo_home.mkdir()
            (cargo_home / "config.toml").write_text(
                '[alias]\nclippy = "--help"\nfmt = "--help"\n', encoding="utf-8",
            )
            environment = {
                "HOME": str(base), "CARGO_HOME": str(cargo_home),
                "RUSTUP_HOME": EVIDENCE.os.environ.get("RUSTUP_HOME", str(Path(EVIDENCE.os.environ["HOME"]) / ".rustup")),
                "PATH": EVIDENCE.os.environ["PATH"],
            }
            redirected_commands = (
                (("clippy", "--version"), "clippy "),
                (("fmt", "--version"), "rustfmt "),
                (("clippy", "--", "-D", "warnings"), "clippy "),
                (("fmt", "--all", "--", "--check"), "rustfmt "),
            )
            for arguments, expected_prefix in redirected_commands:
                dispatched = subprocess.run(
                    ("cargo", *arguments), cwd=root, env=environment,
                    text=True, capture_output=True,
                )
                if dispatched.returncode == 0:
                    self.assertTrue(dispatched.stdout.startswith("Rust's package manager"))
                    self.assertFalse(dispatched.stdout.startswith(expected_prefix))
                else:
                    self.assertIn("alias", dispatched.stderr.lower())
            with patch.object(EVIDENCE, "ROOT", root), patch.dict(EVIDENCE.os.environ, environment, clear=True):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, r"Cargo config aliases for clippy/fmt"):
                    collector_environment(Path(directory) / "isolated-cargo-target")

    def test_clean_node_modules_repairs_tampered_dependency_before_frozen_install(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = base / "repository"
            dependency = base / "fixture-dependency"
            root.mkdir()
            dependency.mkdir()
            (dependency / "package.json").write_text(
                '{"name":"fixture-dependency","version":"1.0.0","main":"index.js"}', encoding="utf-8",
            )
            trusted_bytes = "trusted dependency bytes\n"
            (dependency / "index.js").write_text(trusted_bytes, encoding="utf-8")
            (root / "package.json").write_text(
                '{"name":"fixture-root","version":"1.0.0",'
                '"dependencies":{"fixture-dependency":"file:../fixture-dependency"}}',
                encoding="utf-8",
            )

            with patch.object(EVIDENCE, "ROOT", root), patch.object(
                EVIDENCE, "detect_java_home", return_value="/validated/JDK21",
            ), patch.dict(EVIDENCE.os.environ, {
                "HOME": str(base), "CARGO_HOME": str(base / "parent-cargo-home"),
                "PATH": EVIDENCE.os.environ["PATH"],
            }, clear=True):
                environment = collector_environment(base / "isolated-cargo-target")
                environment["PATH"] = EVIDENCE.os.environ["PATH"]

            def bun_install(*args: str) -> subprocess.CompletedProcess[str]:
                return subprocess.run(
                    ("bun", "install", *args), cwd=root, env=environment,
                    text=True, capture_output=True,
                )

            initial = bun_install()
            self.assertEqual(initial.returncode, 0, initial.stdout + initial.stderr)
            installed_file = root / "node_modules/fixture-dependency/index.js"
            self.assertEqual(installed_file.read_text(encoding="utf-8"), trusted_bytes)
            installed_file.unlink()
            installed_file.write_text("tampered ignored dependency bytes\n", encoding="utf-8")
            self.assertEqual((dependency / "index.js").read_text(encoding="utf-8"), trusted_bytes)

            with patch.object(EVIDENCE, "ROOT", root):
                EVIDENCE.clean_existing_node_modules()
            self.assertFalse((root / "node_modules").exists())
            frozen_clean = bun_install("--frozen-lockfile")
            self.assertEqual(frozen_clean.returncode, 0, frozen_clean.stdout + frozen_clean.stderr)
            self.assertEqual(installed_file.read_text(encoding="utf-8"), trusted_bytes)

    def test_cargo_source_replacement_rejects_local_and_alternate_registries(self) -> None:
        local_source = (
            '[source.crates-io]\nreplace-with = "proof-vendor"\n'
            '[source.proof-vendor]\ndirectory = "vendor"\n'
        )
        alternate_registry = (
            '[source.crates-io]\nreplace-with = "proof-mirror"\n'
            '[source.proof-mirror]\nregistry = "sparse+https://example.invalid/index/"\n'
        )
        cases = (
            ("repo", "config.toml", local_source),
            ("ancestor", "config", alternate_registry),
            ("cargo_home", "config.toml", local_source),
            ("relative_cargo_home", "config", alternate_registry),
        )
        for location, filename, content in cases:
            with self.subTest(location=location), tempfile.TemporaryDirectory() as directory:
                base = Path(directory)
                root = base / "repository"
                config_parent = {
                    "repo": root / ".cargo",
                    "ancestor": base / ".cargo",
                    "cargo_home": root / "absolute-cargo-home",
                    "relative_cargo_home": root / "relative-cargo-home",
                }[location]
                config_parent.mkdir(parents=True)
                (config_parent / filename).write_text(content, encoding="utf-8")
                cargo_home = {
                    "repo": str(base / "unused-cargo-home"),
                    "ancestor": str(base / "unused-cargo-home"),
                    "cargo_home": str(root / "absolute-cargo-home"),
                    "relative_cargo_home": "relative-cargo-home",
                }[location]
                with patch.object(EVIDENCE, "ROOT", root), patch.dict(EVIDENCE.os.environ, {
                    "HOME": str(base), "CARGO_HOME": cargo_home, "PATH": "/usr/bin",
                }, clear=True):
                    with self.assertRaisesRegex(EVIDENCE.EvidenceError, r"Cargo config source replacement"):
                        collector_environment(Path(directory) / "isolated-cargo-target")

    def test_collector_rejects_non_macos_15_before_preparation(self) -> None:
        with (
            patch.object(EVIDENCE.platform, "system", return_value="Darwin"),
            patch.object(EVIDENCE.platform, "machine", return_value="arm64"),
            patch.object(EVIDENCE, "macos_product_version", return_value="14.7.8"),
            patch.object(EVIDENCE.subprocess, "run") as run,
        ):
            with self.assertRaisesRegex(EVIDENCE.EvidenceError, "requires macOS 15"):
                EVIDENCE.collect(REPOSITORY, 7, publish=False)
        run.assert_not_called()

    def test_rejects_git_assume_unchanged_and_skip_worktree_index_flags(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.name", "Test"], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.invalid"], check=True)
            tracked = root / "tracked.txt"
            tracked.write_text("head\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(root), "add", "tracked.txt"], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "baseline"], check=True)

            with patch.object(EVIDENCE, "ROOT", root):
                for flag_args, status_hidden in (
                    (("--assume-unchanged",), True),
                    (("--skip-worktree",), False),
                ):
                    tracked.write_text("hidden local change\n", encoding="utf-8")
                    subprocess.run(["git", "-C", str(root), "update-index", *flag_args, "tracked.txt"], check=True)
                    if status_hidden:
                        self.assertEqual(EVIDENCE.git_status(), "")
                    else:
                        self.assertNotEqual(EVIDENCE.run_git("ls-files", "-t"), "H tracked.txt")
                    for checkpoint in ("collection_start", "collection_end"):
                        with self.subTest(index_flag=flag_args, checkpoint=checkpoint), self.assertRaisesRegex(
                            EVIDENCE.EvidenceError, "nonstandard Git index flags",
                        ):
                            EVIDENCE.require_standard_index_flags()
                    subprocess.run(
                        ["git", "-C", str(root), "update-index", "--no-assume-unchanged", "tracked.txt"],
                        check=True,
                    )
                    subprocess.run(
                        ["git", "-C", str(root), "update-index", "--no-skip-worktree", "tracked.txt"],
                        check=True,
                    )
                    tracked.write_text("head\n", encoding="utf-8")

    def test_raw_tracked_bytes_bind_clean_checkout_and_detect_later_source_change(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.name", "Test"], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.invalid"], check=True)
            regular = root / "source.txt"
            executable = root / "tool.sh"
            regular.write_bytes(b"HEAD bytes\n")
            executable.write_bytes(b"#!/bin/sh\nexit 0\n")
            executable.chmod(0o755)
            (root / "source-link").symlink_to("source.txt")
            subprocess.run(["git", "-C", str(root), "add", "source.txt", "tool.sh", "source-link"], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "baseline"], check=True)
            head = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"], check=True, text=True, capture_output=True,
            ).stdout.strip()
            with patch.object(EVIDENCE, "ROOT", root):
                initial = EVIDENCE.head_worktree_bytes_digest(head)
                self.assertTrue(initial)
                regular.write_bytes(b"changed after collection start\n")
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "tracked worktree bytes differ from HEAD"):
                    EVIDENCE.head_worktree_bytes_digest(head)

    @unittest.skipUnless(os.name == "posix", "POSIX Git filter fixture requires executable scripts")
    def test_raw_tree_digest_uses_bound_git_against_path_shadow_and_clean_smudge_filters(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = base / "repository"
            fake_bin = base / "fake-bin"
            root.mkdir()
            fake_bin.mkdir()
            real_git = Path("/usr/bin/git")

            def git(*arguments: str) -> subprocess.CompletedProcess[str]:
                return subprocess.run(
                    (str(real_git), "-C", str(root), *arguments),
                    check=True, text=True, capture_output=True,
                )

            subprocess.run((str(real_git), "init", "-q", str(root)), check=True)
            git("config", "user.name", "Filter fixture")
            git("config", "user.email", "filter@example.invalid")
            clean_filter = base / "clean-filter.sh"
            smudge_filter = base / "smudge-filter.sh"
            clean_filter.write_text("#!/bin/sh\nprintf 'normalized HEAD blob\\n'\n", encoding="utf-8")
            smudge_filter.write_text("#!/bin/sh\nprintf 'raw worktree bytes\\n'\n", encoding="utf-8")
            clean_filter.chmod(0o755)
            smudge_filter.chmod(0o755)
            git("config", "filter.rawcheck.clean", str(clean_filter))
            git("config", "filter.rawcheck.smudge", str(smudge_filter))
            git("config", "filter.rawcheck.required", "true")
            attributes = root / ".gitattributes"
            attributes.write_text("source.txt filter=rawcheck\n", encoding="utf-8")
            git("add", ".gitattributes")
            git("commit", "-qm", "configure raw tree filter")
            tracked = root / "source.txt"
            tracked.write_bytes(b"raw worktree bytes\n")
            git("add", "source.txt")
            git("commit", "-qm", "add filtered source")
            head = git("rev-parse", "HEAD").stdout.strip()
            tracked.unlink()
            git("restore", "--source=HEAD", "--worktree", "--", "source.txt")
            self.assertEqual(tracked.read_bytes(), b"raw worktree bytes\n")
            self.assertEqual(git("status", "--porcelain", "--untracked-files=all").stdout, "")
            stored_blob = git("cat-file", "blob", "HEAD:source.txt").stdout.encode()
            self.assertEqual(stored_blob, b"normalized HEAD blob\n")
            self.assertNotEqual(stored_blob, tracked.read_bytes())

            fake_git_marker = base / "fake-git-ran"
            fake_git = fake_bin / "git"
            fake_git.write_text(
                "#!/bin/sh\n"
                f"if [ \"$1\" = \"--no-replace-objects\" ] && [ \"$2\" = \"ls-tree\" ]; then printf called > '{fake_git_marker}'; exit 0; fi\n"
                f"exec '{real_git}' \"$@\"\n",
                encoding="utf-8",
            )
            fake_git.chmod(0o755)
            shadow_environment = {
                **os.environ,
                "PATH": os.pathsep.join((str(fake_bin), "/usr/bin", "/bin", "/usr/sbin", "/sbin")),
            }
            with patch.object(EVIDENCE, "ROOT", root), patch.object(
                EVIDENCE.platform, "system", return_value="Darwin",
            ), patch.dict(EVIDENCE.os.environ, shadow_environment, clear=True):
                try:
                    EVIDENCE.head_worktree_bytes_digest(head)
                except EVIDENCE.EvidenceError as error:
                    self.assertRegex(str(error), "tracked worktree bytes differ from HEAD")
                else:
                    self.assertTrue(fake_git_marker.exists(), "legacy raw-tree Git should be shadowed in this fixture")
                    self.fail("raw tree digest accepted worktree bytes that differ from the clean-filtered HEAD blob")
            self.assertFalse(fake_git_marker.exists(), "raw tree must use the bound system Git executable")

    def test_git_replace_commit_cannot_change_tree_behind_same_clean_head_sha(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.name", "Test"], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.invalid"], check=True)
            tracked = root / "tracked.txt"
            tracked.write_text("base bytes\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(root), "add", "tracked.txt"], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "base"], check=True)
            base_head = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"], check=True, text=True, capture_output=True,
            ).stdout.strip()
            tracked.write_text("replacement bytes\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(root), "commit", "-qam", "replacement"], check=True)
            replacement_head = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"], check=True, text=True, capture_output=True,
            ).stdout.strip()
            subprocess.run(["git", "-C", str(root), "replace", base_head, replacement_head], check=True)
            branch = subprocess.run(
                ["git", "-C", str(root), "branch", "--show-current"], check=True, text=True, capture_output=True,
            ).stdout.strip()
            subprocess.run(["git", "-C", str(root), "update-ref", f"refs/heads/{branch}", base_head], check=True)
            self.assertEqual(subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"], check=True, text=True, capture_output=True,
            ).stdout.strip(), base_head)
            self.assertEqual(subprocess.run(
                ["git", "-C", str(root), "status", "--porcelain", "--untracked-files=all"],
                check=True, text=True, capture_output=True,
            ).stdout, "")
            with patch.object(EVIDENCE, "ROOT", root):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "Git replacement refs invalidate"):
                    EVIDENCE.head_worktree_bytes_digest(base_head)

    def test_raw_tracked_bytes_reject_clean_smudge_filter_hidden_by_status(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.name", "Test"], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.invalid"], check=True)
            subprocess.run(
                ["git", "-C", str(root), "config", "filter.proof.clean", "sed 's/worktree/blob/g'"], check=True,
            )
            subprocess.run(
                ["git", "-C", str(root), "config", "filter.proof.smudge", "sed 's/blob/worktree/g'"], check=True,
            )
            (root / ".gitattributes").write_text("filtered.txt filter=proof\n", encoding="utf-8")
            (root / "filtered.txt").write_text("worktree bytes\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(root), "add", ".gitattributes", "filtered.txt"], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "filtered baseline"], check=True)
            head = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"], check=True, text=True, capture_output=True,
            ).stdout.strip()
            with patch.object(EVIDENCE, "ROOT", root):
                self.assertEqual(EVIDENCE.git_status(), "")
                self.assertEqual(EVIDENCE.run_git("ls-files", "-v"), "H .gitattributes\nH filtered.txt")
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "tracked worktree bytes differ from HEAD"):
                    EVIDENCE.head_worktree_bytes_digest(head)

    def test_raw_tracked_bytes_reject_gitlinks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.name", "Test"], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.invalid"], check=True)
            tracked = root / "tracked.txt"
            tracked.write_text("tracked\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(root), "add", "tracked.txt"], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "baseline"], check=True)
            head = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"], check=True, text=True, capture_output=True,
            ).stdout.strip()
            blob = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD:tracked.txt"], check=True, text=True, capture_output=True,
            ).stdout.strip()
            subprocess.run(
                ["git", "-C", str(root), "update-index", "--add", "--cacheinfo", f"160000,{blob},submodule"],
                check=True,
            )
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "gitlink"], check=True)
            gitlink_head = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"], check=True, text=True, capture_output=True,
            ).stdout.strip()
            with patch.object(EVIDENCE, "ROOT", root):
                self.assertNotEqual(head, gitlink_head)
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "gitlinks or unsupported"):
                    EVIDENCE.head_worktree_bytes_digest(gitlink_head)

    def test_rust_tools_must_match_official_stable_manifest(self) -> None:
        current = {
            "rustc": f"rustc {STABLE_RUST['rustc']}",
            "cargo": f"cargo {STABLE_RUST['cargo_cli']}",
            "clippy": f"clippy {STABLE_RUST['clippy_cli']}",
            "rustfmt": f"rustfmt {STABLE_RUST['rustfmt_cli']}",
        }
        EVIDENCE.validate_stable_rust_tools(current, STABLE_RUST)
        rejected = (
            {**current, "rustc": "rustc 1.91.0-nightly (nightly-hash 2025-09-14)"},
            {**current, "rustc": "rustc 1.91.0-beta (beta-hash 2025-09-14)"},
            {**current, "rustc": "rustc 1.89.0 (older-stable-hash 2025-08-01)"},
            {**current, "cargo": "cargo 1.98.0 (olderhash1 2026-08-20)"},
            {**current, "cargo": "cargo 0.100.0 (5f94df478 2026-08-27)"},
            {**current, "clippy": "clippy 0.1.98 (olderhash 2026-08-20)"},
            {**current, "rustfmt": "rustfmt 1.9.0-stable (olderhash 2026-08-20)"},
        )
        for tools in rejected:
            with self.subTest(tools=tools):
                with self.assertRaises(EVIDENCE.EvidenceError):
                    EVIDENCE.validate_stable_rust_tools(tools, STABLE_RUST)

    @unittest.skipUnless(os.name == "posix", "Cargo component dispatch fixture requires POSIX executables")
    def test_cargo_dispatch_probes_path_selected_clippy_and_rustfmt(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            bin_dir = Path(directory)
            for executable, output in (
                ("cargo-clippy", "clippy 0.1.98 (stale 2026-08-20)"),
                ("cargo-fmt", "rustfmt 1.9.0-stable (stale 2026-08-20)"),
            ):
                path = bin_dir / executable
                path.write_text(f"#!/bin/sh\nprintf '%s\\n' '{output}'\n", encoding="utf-8")
                path.chmod(0o755)
            environment = {**os.environ, "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
            binding = EVIDENCE.CommandBinding(
                "/bin", {}, {}, RUST_COMPONENT_DIGESTS, fixture_host_tool_proof(),
            )
            just = EVIDENCE.JustBinding(
                Path("/trusted/just"), "just 1.40.0", "1.40.0", "just-1.40.0-aarch64-apple-darwin.tar.gz",
                "https://github.com/casey/just/releases/download/1.40.0/just-1.40.0-aarch64-apple-darwin.tar.gz",
                "a" * 64, "b" * 64,
            )
            with patch.object(EVIDENCE, "assert_command_binding"):
                selected = EVIDENCE.cargo_component_versions(environment, binding, just)
            self.assertEqual(selected["clippy"], "clippy 0.1.98 (stale 2026-08-20)")
            self.assertEqual(selected["rustfmt"], "rustfmt 1.9.0-stable (stale 2026-08-20)")
            with self.assertRaises(EVIDENCE.EvidenceError):
                EVIDENCE.validate_stable_rust_tools({**selected, **{
                    "rustc": f"rustc {STABLE_RUST['rustc']}", "cargo": f"cargo {STABLE_RUST['cargo_cli']}",
                }}, STABLE_RUST)

    def test_parses_rustc_and_cargo_versions_from_official_channel_fixture(self) -> None:
        fixture = '''\
[pkg.rustc]
version = "1.99.0 (b940084d7 2026-09-28)"
git_commit_hash = "b940084d7eb6a299eb4bfeb8e34901bc051e7ac4"

[pkg.rustc.target.aarch64-apple-darwin]
available = true

[pkg.cargo]
version = "0.100.0 (5f94df478 2026-08-27)"

[pkg.clippy-preview]
version = "0.1.99"
git_commit_hash = "b940084d7eb6a299eb4bfeb8e34901bc051e7ac4"

[pkg.rustfmt-preview]
version = "1.10.0"
git_commit_hash = "b940084d7eb6a299eb4bfeb8e34901bc051e7ac4"
'''
        self.assertEqual(EVIDENCE.parse_stable_manifest_versions(fixture), STABLE_RUST)

    def test_missing_proof_skips_official_manifest_network_lookup(self) -> None:
        with patch.object(EVIDENCE, "fetch_stable_manifest_versions") as fetch_manifest:
            self.assertFalse(EVIDENCE.verify_from_api(
                REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                fetch=snapshots(None), now=NOW, expected_workflow_digest="d" * 64,
            ))
        fetch_manifest.assert_not_called()

    def test_manifest_derives_cli_cargo_version_from_rust_release_not_internal_version(self) -> None:
        fixture = '''\
[pkg.rustc]
version = "1.99.0 (b940084d7 2026-09-28)"
git_commit_hash = "b940084d7eb6a299eb4bfeb8e34901bc051e7ac4"

[pkg.cargo]
version = "0.100.0 (5f94df478 2026-08-27)"

[pkg.clippy-preview]
version = "0.1.99"
git_commit_hash = "b940084d7eb6a299eb4bfeb8e34901bc051e7ac4"

[pkg.rustfmt-preview]
version = "1.10.0"
git_commit_hash = "b940084d7eb6a299eb4bfeb8e34901bc051e7ac4"
'''
        parsed = EVIDENCE.parse_stable_manifest_versions(fixture)
        self.assertEqual(parsed["cargo_cli"], "1.99.0 (5f94df478 2026-08-27)")
        self.assertEqual(parsed["clippy_cli"], STABLE_RUST["clippy_cli"])
        self.assertEqual(parsed["rustfmt_cli"], STABLE_RUST["rustfmt_cli"])
        mismatched_component = fixture.replace(
            'git_commit_hash = "b940084d7eb6a299eb4bfeb8e34901bc051e7ac4"\n\n[pkg.rustfmt-preview]',
            'git_commit_hash = "0000000000000000000000000000000000000000"\n\n[pkg.rustfmt-preview]',
        )
        with self.assertRaises(EVIDENCE.EvidenceError):
            EVIDENCE.parse_stable_manifest_versions(mismatched_component)
        for cargo in (
            "cargo 0.100.0 (5f94df478 2026-08-27)",
            "cargo 1.98.0 (5f94df478 2026-08-27)",
            "cargo 1.99.0-nightly (5f94df478 2026-08-27)",
        ):
            with self.subTest(cargo=cargo), self.assertRaises(EVIDENCE.EvidenceError):
                EVIDENCE.validate_stable_rust_tools(
                    {"rustc": "rustc 1.99.0 (b940084d7 2026-09-28)", "cargo": cargo,
                     "clippy": f"clippy {STABLE_RUST['clippy_cli']}",
                     "rustfmt": f"rustfmt {STABLE_RUST['rustfmt_cli']}"}, parsed
                )

    def test_official_rust_archives_bind_installed_tool_bytes_and_component_tree(self) -> None:
        components, archives = fixture_rust_archives()
        proof = EVIDENCE.derive_rust_component_proof(components, archives)
        self.assertEqual(set(proof.components), set(EVIDENCE.RUST_COMPONENT_PACKAGES))
        self.assertEqual(set(proof.executables), set(EVIDENCE.RUST_EXECUTABLE_PATHS))
        with tempfile.TemporaryDirectory() as temporary:
            rust_root = Path(temporary)
            for relative, (executable_bits, _digest) in proof.files.items():
                path = rust_root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                data = next(
                    content for name, files in {
                        "rustc": {"bin/rustc": b"official-rustc"},
                        "cargo": {"bin/cargo": b"official-cargo"},
                        "clippy": {"bin/cargo-clippy": b"official-cargo-clippy", "bin/clippy-driver": b"official-clippy-driver"},
                        "rustfmt": {"bin/rustfmt": b"official-rustfmt"},
                        "rust-std": {"lib/rustlib/aarch64-apple-darwin/lib/libstd.rlib": b"official-std"},
                    }.items() for candidate, content in files.items() if candidate == relative
                )
                path.write_bytes(data)
                path.chmod(0o644 | executable_bits)
            EVIDENCE.verify_installed_rust_components(rust_root, proof)
            (rust_root / "bin/cargo").write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            with self.assertRaisesRegex(EVIDENCE.EvidenceError, "differs from official"):
                EVIDENCE.verify_installed_rust_components(rust_root, proof)

    def test_stable_manifest_binds_exact_component_archives_for_native_target(self) -> None:
        components, _archives = fixture_rust_archives()
        by_name = {component.name: component for component in components}
        rows = ['date = "2026-10-01"', "[pkg.rustc]", 'version = "1.99.0 (b940084d7 2026-09-28)"']
        for name, package in EVIDENCE.RUST_COMPONENT_PACKAGES.items():
            component = by_name[name]
            rows.extend((
                f"[pkg.{package}.target.{EVIDENCE.RUST_HOST}]",
                f'xz_url = "{component.url}"',
                f'xz_hash = "{component.xz_sha256}"',
            ))
        parsed = EVIDENCE.parse_stable_rust_components("\n".join(rows))
        self.assertEqual(parsed, components)
        invalid = "\n".join(rows).replace("static.rust-lang.org", "attacker.example", 1)
        with self.assertRaises(EVIDENCE.EvidenceError):
            EVIDENCE.parse_stable_rust_components(invalid)

    def test_official_rust_component_archive_digest_and_inventory_fail_closed(self) -> None:
        components, archives = fixture_rust_archives()
        corrupted = dict(archives)
        corrupted["cargo"] = corrupted["cargo"][:-1] + bytes([corrupted["cargo"][-1] ^ 1])
        with self.assertRaisesRegex(EVIDENCE.EvidenceError, "archive digest"):
            EVIDENCE.derive_rust_component_proof(components, corrupted)
        duplicate = dict(archives)
        component = next(item for item in components if item.name == "cargo")
        package_dir = EVIDENCE.RUST_COMPONENT_ARCHIVE_DIRS["cargo"]
        top = f"{package_dir}-1.99.0-{EVIDENCE.RUST_HOST}"
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode="w:xz") as bundle:
            for directory in (top, f"{top}/{package_dir}"):
                member = tarfile.TarInfo(directory)
                member.type = tarfile.DIRTYPE
                bundle.addfile(member)
            manifest = b"file:bin/cargo\nfile:bin/cargo\n"
            member = tarfile.TarInfo(f"{top}/{package_dir}/manifest.in")
            member.size = len(manifest)
            bundle.addfile(member, io.BytesIO(manifest))
            data = b"official-cargo"
            member = tarfile.TarInfo(f"{top}/{package_dir}/bin/cargo")
            member.mode = 0o755
            member.size = len(data)
            bundle.addfile(member, io.BytesIO(data))
        duplicate["cargo"] = output.getvalue()
        replacement = tuple(
            EVIDENCE.RustComponent(item.name, item.url,
                                   EVIDENCE.sha256_bytes(duplicate["cargo"]) if item.name == "cargo" else item.xz_sha256)
            for item in components
        )
        with self.assertRaisesRegex(EVIDENCE.EvidenceError, "repeats a file"):
            EVIDENCE.derive_rust_component_proof(replacement, duplicate)
        for unsafe_member in (f"{top}/../escape", f"{top}/{package_dir}/bin/cargo-link"):
            unsafe = io.BytesIO()
            with tarfile.open(fileobj=unsafe, mode="w:xz") as bundle:
                for directory in (top, f"{top}/{package_dir}"):
                    member = tarfile.TarInfo(directory)
                    member.type = tarfile.DIRTYPE
                    bundle.addfile(member)
                manifest = b"file:bin/cargo\n"
                member = tarfile.TarInfo(f"{top}/{package_dir}/manifest.in")
                member.size = len(manifest)
                bundle.addfile(member, io.BytesIO(manifest))
                member = tarfile.TarInfo(f"{top}/{package_dir}/bin/cargo")
                member.mode = 0o755
                member.size = len(data)
                bundle.addfile(member, io.BytesIO(data))
                member = tarfile.TarInfo(unsafe_member)
                member.type = tarfile.SYMTYPE
                member.linkname = "/tmp/untrusted"
                bundle.addfile(member)
            unsafe_archives = dict(archives)
            unsafe_archives["cargo"] = unsafe.getvalue()
            unsafe_components = tuple(
                EVIDENCE.RustComponent(item.name, item.url,
                    EVIDENCE.sha256_bytes(unsafe_archives["cargo"]) if item.name == "cargo" else item.xz_sha256)
                for item in components
            )
            with self.subTest(member=unsafe_member), self.assertRaisesRegex(
                EVIDENCE.EvidenceError, "unsafe member",
            ):
                EVIDENCE.derive_rust_component_proof(unsafe_components, unsafe_archives)

    def test_manifest_fetch_failure_falls_back_to_hosted_ci(self) -> None:
        with patch.object(EVIDENCE, "fetch_stable_manifest_versions", side_effect=EVIDENCE.EvidenceError("offline")):
            self.assertFalse(EVIDENCE.verify_from_api(
                REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                fetch=snapshots(valid_comment()), now=NOW, expected_workflow_digest="d" * 64,
            ))

    def test_rust_component_archive_failure_falls_back_to_hosted_ci(self) -> None:
        with patch.object(
            EVIDENCE, "fetch_official_rust_component_proof",
            side_effect=EVIDENCE.EvidenceError("official Rust archive unavailable"),
        ):
            self.assertFalse(EVIDENCE.verify_from_api(
                REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                fetch=snapshots(valid_comment()), now=NOW, expected_workflow_digest="d" * 64,
            ))

    def test_rejects_expired_or_edited_comment_api_change_or_nonincorporated_base(self) -> None:
        good = valid_comment()
        old = {**good, "created_at": "2027-01-14T07:00:00Z", "updated_at": "2027-01-14T07:00:00Z"}
        edited = {**good, "updated_at": "2027-01-15T09:00:00Z"}
        self.assertFalse(EVIDENCE.verify_from_api(REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=snapshots(old), now=NOW, expected_workflow_digest="d" * 64))
        self.assertFalse(EVIDENCE.verify_from_api(REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=snapshots(edited), now=NOW, expected_workflow_digest="d" * 64))
        self.assertFalse(EVIDENCE.verify_from_api(REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=snapshots(good, changed_second=True), now=NOW, expected_workflow_digest="d" * 64))
        compare = {"merge_base_commit": {"sha": "f" * 40}}
        fetch = snapshots(good)
        def unmerged(path: str):
            return compare if path.startswith(f"repos/{REPOSITORY}/compare/") else fetch(path)
        self.assertFalse(EVIDENCE.verify_from_api(REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=unmerged, now=NOW, expected_workflow_digest="d" * 64))

    def test_api_failure_is_a_fallback_result(self) -> None:
        self.assertFalse(EVIDENCE.verify_from_api(REPOSITORY, 7, fetch=snapshots(None, api_fail=True)))

    def test_fresh_bun_release_metadata_and_archive_fail_closed_to_hosted_ci(self) -> None:
        good = valid_comment()
        bad_release = copy.deepcopy(BUN_RELEASE)
        bad_release["tag_name"] = "bun-v1.4.1"
        with self.subTest(reason="release metadata mismatch"):
            self.assertFalse(EVIDENCE.verify_from_api(
                REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                fetch=snapshots(good, bun_release=bad_release), now=NOW,
                expected_workflow_digest="d" * 64,
            ))
        with self.subTest(reason="release API unavailable"):
            self.assertFalse(EVIDENCE.verify_from_api(
                REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                fetch=snapshots(good, bun_api_fail=True), now=NOW,
                expected_workflow_digest="d" * 64,
            ))
        with self.subTest(reason="archive download unavailable"), patch.object(
            EVIDENCE.request, "urlopen", side_effect=EVIDENCE.error.URLError("offline"),
        ):
            self.assertFalse(EVIDENCE.verify_from_api(
                REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                fetch=snapshots(good), now=NOW, expected_workflow_digest="d" * 64,
            ))
        with self.subTest(reason="downloaded archive bytes mismatch"), patch.object(
            EVIDENCE.request, "urlopen", return_value=FixtureDownloadResponse(BUN_ARCHIVE + b"tampered"),
        ):
            self.assertFalse(EVIDENCE.verify_from_api(
                REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                fetch=snapshots(good), now=NOW, expected_workflow_digest="d" * 64,
            ))

    def test_receipt_bun_hash_must_match_fresh_official_archive_bytes(self) -> None:
        good = valid_comment()
        payload = json.loads(good["body"].split("\n", 1)[1])
        command = json.loads(payload["tools"]["command_binding"])
        command["executables"]["bun"][1] = "f" * 64
        tools = {
            **payload["tools"], "bun_binary_sha256": "f" * 64,
            "command_binding": json.dumps(command),
        }
        changed = {**good, "body": EVIDENCE.MARKER + "\n" + json.dumps({**payload, "tools": tools})}
        self.assertFalse(EVIDENCE.verify_from_api(
            REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=snapshots(changed), now=NOW, expected_workflow_digest="d" * 64,
        ))

    def test_receipt_rust_tree_and_executable_hashes_must_match_fresh_official_archives(self) -> None:
        original = valid_comment()
        payload = json.loads(original["body"].split("\n", 1)[1])
        for target in ("rust_components", "rustc"):
            command = json.loads(payload["tools"]["command_binding"])
            if target == "rust_components":
                command["rust_components"]["rustc"] = "f" * 64
            else:
                command["executables"]["rustc"][1] = "f" * 64
            tools = {**payload["tools"], "command_binding": json.dumps(command)}
            changed = {**original, "body": EVIDENCE.MARKER + "\n" + json.dumps({**payload, "tools": tools})}
            with self.subTest(target=target):
                self.assertFalse(EVIDENCE.verify_from_api(
                    REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
                    fetch=snapshots(changed), now=NOW, expected_workflow_digest="d" * 64,
                ))

    def test_reads_all_comment_pages_and_rejects_mutation_between_reads(self) -> None:
        good = valid_comment()
        pages: dict[str, int] = {}

        def paginated(path: str):
            if "comments?" not in path:
                return snapshots(good)(path)
            pages[path] = pages.get(path, 0) + 1
            if path.endswith("page=1"):
                return [{"id": n} for n in range(100)]
            return [good]

        # The hundred non-marker comments are valid filler and are byte-identical on both snapshots.
        self.assertTrue(EVIDENCE.verify_from_api(
            REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=paginated, now=NOW, expected_workflow_digest="d" * 64,
        ))
        self.assertEqual(pages.get(f"repos/{REPOSITORY}/issues/7/comments?per_page=100&page=2"), 2)

        edited_once = {**good}
        comment_reads = 0
        def edited_comments(path: str):
            if "comments?" not in path:
                return snapshots(good)(path)
            if path.endswith("page=1"):
                return [{"id": n} for n in range(100)]
            nonlocal comment_reads
            comment_reads += 1
            if comment_reads == 2:
                edited_once["updated_at"] = "2027-01-15T08:01:00Z"
            return [copy.deepcopy(edited_once)]
        self.assertFalse(EVIDENCE.verify_from_api(
            REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=edited_comments, now=NOW, expected_workflow_digest="d" * 64,
        ))

    def test_old_head_proof_does_not_block_a_new_current_head_proof(self) -> None:
        good = valid_comment()
        old_payload = json.loads(good["body"].split("\n", 1)[1])
        old_payload["head_sha"] = "f" * 40
        old = {**good, "id": 40, "body": EVIDENCE.MARKER + "\n" + json.dumps(old_payload)}
        self.assertTrue(EVIDENCE.verify_from_api(
            REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=snapshots([old, good]), now=NOW, expected_workflow_digest="d" * 64,
        ))

    def test_expired_same_head_proof_can_be_replaced_by_fresh_proof(self) -> None:
        good = valid_comment()
        expired_payload = json.loads(good["body"].split("\n", 1)[1])
        expired_payload["completed_at"] = "2027-01-14T07:00:00Z"
        expired = {
            **good, "id": 40, "created_at": expired_payload["completed_at"],
            "updated_at": expired_payload["completed_at"],
            "body": EVIDENCE.MARKER + "\n" + json.dumps(expired_payload),
        }
        self.assertTrue(EVIDENCE.verify_from_api(
            REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=snapshots([expired, good]), now=NOW, expected_workflow_digest="d" * 64,
        ))

    def test_verify_cli_writes_false_then_only_successful_proof_writes_true(self) -> None:
        # The startup false is intentionally preserved if validation raises or the process is interrupted.
        with tempfile.NamedTemporaryFile(mode="r+", encoding="utf-8") as output:
            EVIDENCE.output_reuse(False, output.name)
            self.assertEqual(Path(output.name).read_text(), "reuse=false\n")

    def test_collector_stops_at_first_failed_command_without_publishing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(EVIDENCE.platform, "system", return_value="Darwin"),
                patch.object(EVIDENCE.platform, "machine", return_value="arm64"),
                patch.object(EVIDENCE, "git_status", return_value=""),
                patch.object(EVIDENCE, "run_git", side_effect=[HEAD, BASE]),
                patch.object(EVIDENCE, "workflow_digest", return_value="d" * 64),
                patch.object(EVIDENCE, "gh_token", return_value="existing-token"),
                collector_binding_patches(),
                patch.object(EVIDENCE, "api_request", side_effect=[
                    {"full_name": REPOSITORY, "owner": {"login": "owner"}}, {"login": "owner"},
                    {"number": 7, "state": "open", "base": {"sha": BASE, "ref": "master", "repo": {"full_name": REPOSITORY}},
                     "head": {"sha": HEAD, "repo": {"full_name": REPOSITORY}}},
                ]),
                patch.object(EVIDENCE, "macos_product_version", return_value="15.0"),
                patch.object(EVIDENCE, "require_standard_index_flags"),
                patch.object(EVIDENCE, "head_worktree_bytes_digest", return_value="source-hash"),
                patch.object(EVIDENCE, "tool_versions", return_value={"macos": "15", "architecture": "arm64"}),
                patch.object(EVIDENCE, "detect_java_home", return_value="/validated/JDK21"),
                patch.object(EVIDENCE, "ROOT", Path(directory)),
                patch.object(EVIDENCE.subprocess, "run", return_value=subprocess.CompletedProcess([], 9, "", "failed")),
            ):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "local command failed"):
                    EVIDENCE.collect(REPOSITORY, 7, publish=True)

    def test_collector_rejects_wrong_java_before_quality_commands(self) -> None:
        valid_tools = json.loads(valid_comment()["body"].split("\n", 1)[1])["tools"]
        versions = {**valid_tools,
            "macos": "15.0", "architecture": "arm64", "rustc": "rustc stable",
            "cargo": "cargo stable", "java": 'openjdk version "17.0.13"',
            "java_home": "/Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home",
            "java_vendor": "Eclipse Adoptium",
            "bun": "1.4.2", "just": "just 1.40.0", "graphviz": "dot - graphviz version 12.0.0",
        }
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(EVIDENCE.platform, "system", return_value="Darwin"),
                patch.object(EVIDENCE.platform, "machine", return_value="arm64"),
                patch.object(EVIDENCE, "git_status", return_value=""),
                patch.object(EVIDENCE, "run_git", side_effect=[HEAD, BASE]),
                patch.object(EVIDENCE, "workflow_digest", return_value="d" * 64),
                patch.object(EVIDENCE, "gh_token", return_value="existing-token"),
                collector_binding_patches(),
                patch.object(EVIDENCE, "api_request", side_effect=[
                    {"full_name": REPOSITORY, "owner": {"login": "owner"}}, {"login": "owner"},
                    {"number": 7, "state": "open", "base": {"sha": BASE, "ref": "master", "repo": {"full_name": REPOSITORY}},
                     "head": {"sha": HEAD, "repo": {"full_name": REPOSITORY}}},
                ]),
                patch.object(EVIDENCE, "macos_product_version", return_value="15.0"),
                patch.object(EVIDENCE, "require_standard_index_flags"),
                patch.object(EVIDENCE, "head_worktree_bytes_digest", return_value="source-hash"),
                patch.object(EVIDENCE, "tool_versions", return_value=versions),
                patch.object(EVIDENCE, "detect_java_home", return_value="/validated/JDK21"),
                patch.object(EVIDENCE, "fetch_stable_manifest_versions", return_value=STABLE_RUST),
                patch.object(EVIDENCE, "ROOT", Path(directory)),
                patch.object(EVIDENCE.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "", "")) as run,
            ):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "Java 21"):
                    EVIDENCE.collect(REPOSITORY, 7, publish=True)
                self.assertEqual(run.call_count, len(EVIDENCE.PREPARATION_COMMANDS) - 1)

    def test_collector_rejects_nonstable_rust_before_quality_commands(self) -> None:
        valid_tools = json.loads(valid_comment()["body"].split("\n", 1)[1])["tools"]
        versions = {**valid_tools,
            "macos": "15.0", "architecture": "arm64",
            "rustc": "rustc 1.91.0-nightly (nightly-hash 2025-09-14)",
            "cargo": f"cargo {STABLE_RUST['cargo_cli']}", "java": 'openjdk version "21.0.12"',
            "java_home": "/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home",
            "java_vendor": "Eclipse Adoptium",
            "bun": "1.4.2", "just": "just 1.40.0", "graphviz": "dot - graphviz version 12.0.0",
        }
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(EVIDENCE.platform, "system", return_value="Darwin"),
                patch.object(EVIDENCE.platform, "machine", return_value="arm64"),
                patch.object(EVIDENCE, "git_status", return_value=""),
                patch.object(EVIDENCE, "run_git", side_effect=[HEAD, BASE]),
                patch.object(EVIDENCE, "workflow_digest", return_value="d" * 64),
                patch.object(EVIDENCE, "gh_token", return_value="existing-token"),
                collector_binding_patches(),
                patch.object(EVIDENCE, "api_request", side_effect=[
                    {"full_name": REPOSITORY, "owner": {"login": "owner"}}, {"login": "owner"},
                    {"number": 7, "state": "open", "base": {"sha": BASE, "ref": "master", "repo": {"full_name": REPOSITORY}},
                     "head": {"sha": HEAD, "repo": {"full_name": REPOSITORY}}},
                ]),
                patch.object(EVIDENCE, "macos_product_version", return_value="15.0"),
                patch.object(EVIDENCE, "require_standard_index_flags"),
                patch.object(EVIDENCE, "head_worktree_bytes_digest", return_value="source-hash"),
                patch.object(EVIDENCE, "tool_versions", return_value=versions),
                patch.object(EVIDENCE, "detect_java_home", return_value="/validated/JDK21"),
                patch.object(EVIDENCE, "fetch_stable_manifest_versions", return_value=STABLE_RUST),
                patch.object(EVIDENCE, "ROOT", Path(directory)),
                patch.object(EVIDENCE.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "", "")) as run,
            ):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "current official stable channel"):
                    EVIDENCE.collect(REPOSITORY, 7, publish=True)
                self.assertEqual(run.call_count, len(EVIDENCE.PREPARATION_COMMANDS) - 1)




class TrustedBunTest(unittest.TestCase):
    def test_official_bun_archive_requires_exact_asset_and_safe_regular_binary(self) -> None:
        binding = EVIDENCE.materialize_trusted_bun(BUN_RELEASE, BUN_ARCHIVE)
        self.assertEqual(binding, fixture_bun_binding())

        wrong_digest = copy.deepcopy(BUN_RELEASE)
        wrong_digest["assets"][0]["digest"] = "sha256:" + "f" * 64
        with self.assertRaisesRegex(EVIDENCE.EvidenceError, "digest"):
            EVIDENCE.materialize_trusted_bun(wrong_digest, BUN_ARCHIVE)

        unsafe_archive = io.BytesIO()
        with zipfile.ZipFile(unsafe_archive, "w", compression=zipfile.ZIP_STORED) as output:
            directory = zipfile.ZipInfo("bun-darwin-aarch64/")
            directory.external_attr = (stat.S_IFDIR | 0o755) << 16 | 0x10
            output.writestr(directory, b"")
            executable = zipfile.ZipInfo("../bun")
            executable.external_attr = (stat.S_IFREG | 0o755) << 16
            output.writestr(executable, BUN_BINARY)
        archive = unsafe_archive.getvalue()
        changed_release = copy.deepcopy(BUN_RELEASE)
        changed_release["assets"][0]["size"] = len(archive)
        changed_release["assets"][0]["digest"] = "sha256:" + EVIDENCE.sha256_bytes(archive)
        with self.assertRaisesRegex(EVIDENCE.EvidenceError, "entries"):
            EVIDENCE.materialize_trusted_bun(changed_release, archive)

    @unittest.skipUnless(os.name == "posix", "POSIX child executable fixture is required")
    def test_real_noop_bun_wrapper_cannot_match_official_release_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            wrapper = Path(temporary) / "bun"
            wrapper.write_text(
                "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 1.4.2; fi\nexit 0\n",
                encoding="utf-8",
            )
            wrapper.chmod(0o755)
            version = subprocess.run(
                (str(wrapper), "--version"), capture_output=True, text=True, check=False,
            )
            install = subprocess.run(
                (str(wrapper), "install", "--frozen-lockfile"),
                capture_output=True, text=True, check=False,
            )
            self.assertEqual((version.returncode, version.stdout.strip()), (0, "1.4.2"))
            self.assertEqual(install.returncode, 0)
            with self.assertRaisesRegex(EVIDENCE.EvidenceError, "official Bun release"):
                EVIDENCE.assert_official_bun_binary(wrapper, BUN_BINARY_SHA256)


class TrustedJustTest(unittest.TestCase):
    @staticmethod
    def archive_for(binary: bytes) -> bytes:
        import io
        import tarfile

        archive = io.BytesIO()
        with tarfile.open(fileobj=archive, mode="w:gz") as output:
            info = tarfile.TarInfo("just")
            info.size = len(binary)
            output.addfile(info, io.BytesIO(binary))
        return archive.getvalue()

    @staticmethod
    def release_for(version: str, archive: bytes) -> dict[str, object]:
        import hashlib

        asset_name = f"just-{version}-aarch64-apple-darwin.tar.gz"
        return {
            "tag_name": version,
            "draft": False,
            "prerelease": False,
            "assets": [{
                "name": asset_name,
                "browser_download_url": f"https://github.com/casey/just/releases/download/{version}/{asset_name}",
                "digest": f"sha256:{hashlib.sha256(archive).hexdigest()}",
                "size": len(archive),
            }],
        }

    def test_path_shim_with_plausible_version_and_zero_exit_is_never_run(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            hostile_bin = root / "hostile-bin"
            hostile_bin.mkdir()
            hostile_marker = root / "hostile-ran"
            trusted_marker = root / "trusted-ran"
            shim = hostile_bin / "just"
            shim.write_text(
                f"#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'just 1.40.0'; "
                f"else echo hostile >> '{hostile_marker}'; fi\nexit 0\n",
                encoding="utf-8",
            )
            shim.chmod(0o755)
            trusted_binary = (
                "#!/bin/sh\n"
                "if [ \"$1\" = \"--version\" ]; then echo 'just 1.40.0'; "
                f"else echo trusted >> '{trusted_marker}'; fi\nexit 0\n"
            ).encode()
            archive = self.archive_for(trusted_binary)
            binding = EVIDENCE.materialize_trusted_just(
                self.release_for("1.40.0", archive), archive, root / "trusted", {"PATH": str(hostile_bin)}
            )
            for _command_id, argv in EVIDENCE.COMMANDS:
                bound = EVIDENCE.bind_command(argv, binding)
                if argv[0] == "just":
                    self.assertEqual(bound[0], str(binding.path))
            result = subprocess.run(
                EVIDENCE.bind_command(("just", "fmt-check"), binding),
                env={"PATH": str(hostile_bin), "JUST_TRUSTED_MARKER": str(trusted_marker)},
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0)
            self.assertTrue(trusted_marker.is_file())
            self.assertFalse(hostile_marker.exists())
            EVIDENCE.assert_trusted_just(binding)

    def test_trusted_just_rejects_digest_asset_and_path_replacement(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            binary = b"#!/bin/sh\necho 'just 1.40.0'\n"
            archive = self.archive_for(binary)
            release = self.release_for("1.40.0", archive)
            wrong_digest = copy.deepcopy(release)
            wrong_digest["assets"][0]["digest"] = "sha256:" + "f" * 64
            with self.assertRaisesRegex(EVIDENCE.EvidenceError, "digest"):
                EVIDENCE.materialize_trusted_just(wrong_digest, archive, root / "wrong-digest", {})
            wrong_asset = copy.deepcopy(release)
            wrong_asset["assets"][0]["browser_download_url"] = "https://example.invalid/just"
            with self.assertRaisesRegex(EVIDENCE.EvidenceError, "official release asset"):
                EVIDENCE.materialize_trusted_just(wrong_asset, archive, root / "wrong-asset", {})
            binding = EVIDENCE.materialize_trusted_just(release, archive, root / "tool", {})
            binding.path.write_bytes(b"#!/bin/sh\necho 'just 1.40.0 changed'\n")
            with self.assertRaisesRegex(EVIDENCE.EvidenceError, "changed"):
                EVIDENCE.assert_trusted_just(binding)

    def test_trusted_just_download_network_errors_fail_closed(self) -> None:
        archive = self.archive_for(b"just-placeholder")
        release = self.release_for("1.40.0", archive)
        failures = (
            EVIDENCE.error.URLError("offline"),
            EVIDENCE.error.HTTPError("https://github.com", 503, "unavailable", {}, None),
            TimeoutError("timed out"),
        )
        for failure in failures:
            with self.subTest(failure=type(failure).__name__), patch.object(
                EVIDENCE, "api_request", return_value=release,
            ), patch.object(EVIDENCE.request, "urlopen", side_effect=failure):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "asset download failed"):
                    EVIDENCE.download_trusted_just("token", Path("/tmp/unused"), {"PATH": "/usr/bin"})

    def test_failed_binding_setup_removes_fresh_inputs_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            inputs = root / "krr-local-macos-inputs-failed"
            inputs.mkdir()
            cargo_home = inputs / "cargo-home"; cargo_home.mkdir()
            cargo_target = inputs / "cargo-target"; cargo_target.mkdir()
            bun_cache = inputs / "bun-cache"; bun_cache.mkdir()
            with patch.object(EVIDENCE, "command_environment", return_value={"PATH": "/usr/bin:/bin"}), patch.object(
                EVIDENCE, "download_trusted_just", side_effect=EVIDENCE.EvidenceError("offline"),
            ):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "offline"):
                    EVIDENCE.prepare_command_environment("token", inputs, cargo_target, cargo_home, bun_cache)
            self.assertFalse(inputs.exists())

    def test_version_only_just_proof_is_rejected(self) -> None:
        good = valid_comment()
        payload = json.loads(good["body"].split("\n", 1)[1])
        for key in ("just_release", "just_asset", "just_asset_sha256", "just_binary_sha256", "just_path", "command_binding"):
            payload["tools"].pop(key, None)
        changed = {**good, "body": EVIDENCE.MARKER + "\n" + json.dumps(payload)}
        self.assertFalse(EVIDENCE.verify_from_api(
            REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=snapshots(changed), now=NOW, expected_workflow_digest="d" * 64,
        ))

    @unittest.skipUnless(os.name == "posix", "POSIX production dispatcher fixture is required")
    def test_production_dispatcher_binds_just_and_blocks_real_grep_and_shasum_shadows(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            inputs = root / "krr-local-macos-inputs-fixture"
            just_dir = inputs / "trusted-just"
            just_dir.mkdir(parents=True)
            just_path = just_dir / "just"
            marker = root / "nested-cargo-ran"
            just_path.write_text(f"#!/bin/sh\ncargo nested-recipe\n", encoding="utf-8")
            just_path.chmod(0o755)
            just_sha = EVIDENCE.sha256_bytes(just_path.read_bytes())
            just = EVIDENCE.JustBinding(
                just_path, "just 1.40.0", "1.40.0", "just-1.40.0-aarch64-apple-darwin.tar.gz",
                "https://github.com/casey/just/releases/download/1.40.0/just-1.40.0-aarch64-apple-darwin.tar.gz",
                "a" * 64, just_sha,
            )

            account = root / "account"
            rustup_home = account / ".rustup"
            rust_bin = rustup_home / "toolchains/stable-aarch64-apple-darwin/bin"
            rust_bin.mkdir(parents=True)
            rustup = account / ".cargo/bin/rustup"
            rustup.parent.mkdir(parents=True)
            rustup.write_text(
                "#!/bin/sh\nfor name do selected=$name; done\nprintf '%s\\n' '" + str(rust_bin) + "/'\"$selected\"\n",
                encoding="utf-8",
            )
            rustup.chmod(0o755)
            for name in ("cargo", "rustc", "cargo-clippy", "clippy-driver", "rustfmt"):
                tool = rust_bin / name
                tool.write_text(
                    f"#!/bin/sh\nprintf '%s\\n' 'rust-{name}' >> '{marker}'\n", encoding="utf-8",
                )
                tool.chmod(0o755)
            standard_library = rust_bin.parent / "lib/rustlib/aarch64-apple-darwin/lib/libstd.rlib"
            standard_library.parent.mkdir(parents=True)
            standard_library.write_bytes(b"official-stdlib-fixture")
            rust_proof = fixture_installed_rust_proof(rust_bin.parent)

            homebrew = root / "opt-homebrew"
            brew_bin = homebrew / "bin"
            brew_bin.mkdir(parents=True)
            install_marker = root / "brew-install-env"
            dot_path = brew_bin / "dot"
            dot_path.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            dot_path.chmod(0o755)
            brew_tool = brew_bin / "brew"
            brew_tool.write_text(
                "#!/bin/sh\n"
                f"printf '%s\\n' \"$PATH\" \"$HOMEBREW_NO_AUTO_UPDATE\" \"$*\" > '{install_marker}'\n"
                f"printf '%s\\n' '#!/bin/sh' 'exit 0' > '{dot_path}'\n"
                f"/bin/chmod 755 '{dot_path}'\n",
                encoding="utf-8",
            )
            brew_tool.chmod(0o755)
            for name in ("bun",):
                tool = brew_bin / name
                tool.write_bytes(BUN_BINARY)
                tool.chmod(0o755)
            java_root = root / "java-installations"
            java_home = java_root / "temurin-21.jdk/Contents/Home"
            java = java_home / "bin/java"
            java.parent.mkdir(parents=True)
            java_bytes = b"#!/bin/sh\n# authenticated Java fixture\nexit 0\n"
            java.write_bytes(java_bytes)
            java.chmod(0o755)
            java_library = java_home / "lib/server/libjvm.dylib"
            java_library.parent.mkdir(parents=True)
            java_library.write_bytes(b"authenticated-jvm-fixture")
            graphviz_library = root / "opt-homebrew/Cellar/graphviz/12.0.0/lib/libgvc.dylib"
            graphviz_library.parent.mkdir(parents=True)
            graphviz_library.write_bytes(b"authenticated-graphviz-library-fixture")
            official_dot = (
                b"#!/bin/sh\n"
                b"echo 'dot - graphviz version 12.0.0' >&2\n"
                b"exit 0\n"
            )
            host_proof = fixture_host_tool_proof()
            host_proof = EVIDENCE.TOOL_PROVENANCE.HostToolProof(
                {
                    str(dot_path): EVIDENCE.TOOL_PROVENANCE.InventoryEntry(
                        "file", 0o111, EVIDENCE.sha256_bytes(official_dot),
                    ),
                    str(graphviz_library): EVIDENCE.TOOL_PROVENANCE.InventoryEntry(
                        "file", 0, EVIDENCE.sha256_bytes(graphviz_library.read_bytes()),
                    ),
                },
                {
                    "bin/java": EVIDENCE.TOOL_PROVENANCE.InventoryEntry(
                        "file", 0o111, EVIDENCE.sha256_bytes(java_bytes),
                    ),
                    "lib/server/libjvm.dylib": EVIDENCE.TOOL_PROVENANCE.InventoryEntry(
                        "file", 0, EVIDENCE.sha256_bytes(java_library.read_bytes()),
                    ),
                },
                fixture_host_tool_proof().identities,
                str(dot_path),
            )
            parent_environment = {"RUSTUP_HOME": str(rustup_home), "PATH": str(brew_bin)}
            python_tool = root / "python/bin/python3"
            python_tool.parent.mkdir(parents=True)
            python_tool.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            python_tool.chmod(0o755)
            os_bin = root / "os-tools"
            os_bin.mkdir()
            system_tools = {}
            for name in ("git", "sw_vers", "uname"):
                tool = os_bin / name
                tool.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
                tool.chmod(0o755)
                system_tools[Path("/usr/bin") / name] = tool
            system_tools[Path("/bin/bash")] = Path("/bin/bash").resolve(strict=True)

            original_trusted = EVIDENCE.trusted_executable

            def select_executable(path, roots):
                candidate = Path(path)
                if candidate in system_tools:
                    return system_tools[candidate].resolve(strict=True)
                if candidate == Path(sys.executable):
                    return python_tool.resolve(strict=True)
                return original_trusted(candidate, roots)

            original_run = subprocess.run

            def verify_fixture_host_tools(proof, selected_java_home):
                self.assertEqual(proof, host_proof)
                for raw_path, identity in proof.graphviz_files.items():
                    path = Path(raw_path)
                    if (not path.is_file() or path.is_symlink()
                            or stat.S_IMODE(path.stat().st_mode) & 0o111 != identity.mode
                            or EVIDENCE.sha256_bytes(path.read_bytes()) != identity.value):
                        raise EVIDENCE.EvidenceError(
                            "installed Graphviz fixture differs from authenticated artifact",
                        )
                for relative_path, identity in proof.java_files.items():
                    path = selected_java_home / relative_path
                    if (not path.is_file() or path.is_symlink()
                            or stat.S_IMODE(path.stat().st_mode) & 0o111 != identity.mode
                            or EVIDENCE.sha256_bytes(path.read_bytes()) != identity.value):
                        raise EVIDENCE.EvidenceError(
                            "installed Java fixture differs from authenticated artifact",
                        )

            host_tools_patcher = patch.object(
                EVIDENCE, "verify_installed_host_tools", side_effect=verify_fixture_host_tools,
            )
            host_tools_patcher.start()
            self.addCleanup(host_tools_patcher.stop)

            def select_java_home(arguments, **kwargs):
                if tuple(arguments) == ("/usr/libexec/java_home", "-v", "21"):
                    return subprocess.CompletedProcess(arguments, 0, stdout=str(java_home) + "\n", stderr="")
                return original_run(arguments, **kwargs)

            with patch.object(EVIDENCE, "account_home", return_value=account), patch.object(
                EVIDENCE, "HOMEBREW_ROOT", homebrew,
            ), patch.object(EVIDENCE, "JAVA_INSTALL_ROOT", java_root), patch.object(
                EVIDENCE, "trusted_executable", side_effect=select_executable,
            ), patch.object(EVIDENCE.subprocess, "run", side_effect=select_java_home):
                with self.assertRaises(EVIDENCE.EvidenceError):
                    EVIDENCE.command_binding(parent_environment, just, BUN_BINARY_SHA256, rust_proof, host_proof)
                self.assertFalse(install_marker.exists(), "an unauthenticated Homebrew bootstrap must not run")
                shutil.rmtree(just_dir.parent / "trusted-commands")
                dot_path.write_bytes(official_dot)
                dot_path.chmod(0o755)
                binding = EVIDENCE.command_binding(parent_environment, just, BUN_BINARY_SHA256, rust_proof, host_proof)
                graphviz_library.write_bytes(b"tampered-graphviz-library")
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "authenticated artifact"):
                    EVIDENCE.assert_command_binding(binding, just)
                graphviz_library.write_bytes(b"authenticated-graphviz-library-fixture")
                java_library.write_bytes(b"tampered-jvm-fixture")
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "authenticated artifact"):
                    EVIDENCE.assert_command_binding(binding, just)
                java_library.write_bytes(b"authenticated-jvm-fixture")
                standard_library.write_bytes(b"tampered-stdlib-fixture")
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "differs from official"):
                    EVIDENCE.assert_command_binding(binding, just)
                standard_library.write_bytes(b"official-stdlib-fixture")
                EVIDENCE.assert_command_binding(binding, just)
                wrapper = (
                    b"#!/bin/sh\n# no-op wrapper with a different digest\n"
                    b"if [ \"$1\" = \"--version\" ]; then echo 1.4.2; fi\nexit 0\n"
                )
                (brew_bin / "bun").write_bytes(wrapper)
                alternate_just_dir = inputs / "recheck/trusted-just"
                alternate_just_dir.mkdir(parents=True)
                alternate_just_path = alternate_just_dir / "just"
                shutil.copyfile(just_path, alternate_just_path)
                alternate_just_path.chmod(0o755)
                alternate_just = EVIDENCE.JustBinding(
                    alternate_just_path, just.version, just.release_tag, just.asset_name,
                    just.asset_url, just.asset_sha256, just.binary_sha256,
                )
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "official Bun release"):
                    EVIDENCE.command_binding(parent_environment, alternate_just, BUN_BINARY_SHA256, rust_proof, fixture_host_tool_proof())
                (brew_bin / "bun").write_bytes(BUN_BINARY)
                wrapper_version = original_run(
                    (str(brew_bin / "bun"), "--version"), capture_output=True, text=True, check=False,
                )
                wrapper_install = original_run(
                    (str(brew_bin / "bun"), "install", "--frozen-lockfile"),
                    capture_output=True, text=True, check=False,
                )
                self.assertEqual((wrapper_version.returncode, wrapper_version.stdout.strip()), (0, "1.4.2"))
                self.assertEqual(wrapper_install.returncode, 0)
                self.assertEqual(binding.executables["bun"][1], BUN_BINARY_SHA256)

            self.assertFalse(install_marker.exists(), "existing Graphviz must not trigger Homebrew install")
            self.assertEqual(binding.executables["bash"][0], str(Path("/bin/bash").resolve(strict=True)))
            environment = {"PATH": binding.path}
            EVIDENCE.assert_command_binding(binding, just)
            nested = subprocess.run(
                EVIDENCE.bind_command(("just", "nested-recipe"), just),
                env=environment, text=True, capture_output=True, check=False,
            )
            EVIDENCE.assert_command_binding(binding, just)
            self.assertEqual(nested.returncode, 0, nested.stdout + nested.stderr)
            self.assertEqual(marker.read_text().strip(), "rust-cargo")
            system_bash = subprocess.run(
                (binding.executables["bash"][0], "-c", "printf system-bash"),
                env=environment, text=True, capture_output=True, check=False,
            )
            EVIDENCE.assert_command_binding(binding, just)
            self.assertEqual(system_bash.returncode, 0, system_bash.stderr)
            self.assertEqual(system_bash.stdout, "system-bash")

            shadow_markers = root / "shadow-executed"
            (brew_bin / "grep").write_text(
                f"#!/bin/sh\nprintf '%s\\n' grep >> '{shadow_markers}'\nexit 1\n", encoding="utf-8",
            )
            (brew_bin / "grep").chmod(0o755)
            (brew_bin / "shasum").write_text(
                f"#!/bin/sh\nprintf '%s\\n' shasum >> '{shadow_markers}'\nexit 0\n", encoding="utf-8",
            )
            (brew_bin / "shasum").chmod(0o755)
            grep_check = ("/bin/sh", "-c", "if printf 'forbidden egui\\n' | grep -E 'forbidden[[:space:]]+egui'; then exit 1; fi")
            checksums = root / "bad.sha256"
            (root / "prohibited.txt").write_text("wrong checksum\n", encoding="utf-8")
            checksums.write_text("0" * 64 + "  prohibited.txt\n", encoding="utf-8")
            checksum_check = ("shasum", "-a", "256", "-c", str(checksums))
            old_environment = {"PATH": os.pathsep.join((str(brew_bin), *EVIDENCE.SYSTEM_COMMAND_PATH))}
            for command in (grep_check, checksum_check):
                old_result = subprocess.run(command, cwd=root, env=old_environment, text=True, capture_output=True)
                self.assertEqual(old_result.returncode, 0, old_result.stdout + old_result.stderr)
            self.assertEqual(shadow_markers.read_text().splitlines(), ["grep", "shasum"])
            shadow_markers.unlink()

            for command in (grep_check, checksum_check):
                EVIDENCE.assert_command_binding(binding, just)
                safe_result = subprocess.run(command, cwd=root, env=environment, text=True, capture_output=True)
                EVIDENCE.assert_command_binding(binding, just)
                self.assertNotEqual(safe_result.returncode, 0, safe_result.stdout + safe_result.stderr)
            self.assertFalse(shadow_markers.exists())
            EVIDENCE.assert_command_binding(binding, just)

            def fixture_just(name: str) -> EVIDENCE.JustBinding:
                directory = root / name / "trusted-just"
                directory.mkdir(parents=True)
                executable = directory / "just"
                executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
                executable.chmod(0o755)
                return EVIDENCE.JustBinding(
                    executable, "just 1.40.0", "1.40.0", "just-1.40.0-aarch64-apple-darwin.tar.gz",
                    "https://github.com/casey/just/releases/download/1.40.0/just-1.40.0-aarch64-apple-darwin.tar.gz",
                    "a" * 64, EVIDENCE.sha256_bytes(executable.read_bytes()),
                )

            dot_path.unlink()
            provisioned_just = fixture_just("missing-dot-inputs")
            with patch.object(EVIDENCE, "account_home", return_value=account), patch.object(
                EVIDENCE, "HOMEBREW_ROOT", homebrew,
            ), patch.object(EVIDENCE, "JAVA_INSTALL_ROOT", java_root), patch.object(
                EVIDENCE, "trusted_executable", side_effect=select_executable,
            ), patch.object(EVIDENCE.subprocess, "run", side_effect=select_java_home):
                with self.assertRaises(EVIDENCE.EvidenceError):
                    EVIDENCE.command_binding(
                        parent_environment, provisioned_just, BUN_BINARY_SHA256, rust_proof, host_proof,
                    )
            self.assertFalse(install_marker.exists(), "missing Graphviz must not run Homebrew")

            outside_dot = root / "outside-dot"
            outside_dot.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            outside_dot.chmod(0o755)
            dot_path.symlink_to(outside_dot)
            invalid_just = fixture_just("invalid-dot-inputs")
            with patch.object(EVIDENCE, "account_home", return_value=account), patch.object(
                EVIDENCE, "HOMEBREW_ROOT", homebrew,
            ), patch.object(EVIDENCE, "JAVA_INSTALL_ROOT", java_root), patch.object(
                EVIDENCE, "trusted_executable", side_effect=select_executable,
            ), patch.object(EVIDENCE.subprocess, "run", side_effect=select_java_home):
                with self.assertRaises(EVIDENCE.EvidenceError):
                    EVIDENCE.command_binding(parent_environment, invalid_just, BUN_BINARY_SHA256, rust_proof, host_proof)
            self.assertFalse(install_marker.exists())

            dot_path.unlink()
            dot_path.symlink_to(root / "missing-dot-target")
            broken_just = fixture_just("broken-dot-inputs")
            with patch.object(EVIDENCE, "account_home", return_value=account), patch.object(
                EVIDENCE, "HOMEBREW_ROOT", homebrew,
            ), patch.object(EVIDENCE, "JAVA_INSTALL_ROOT", java_root), patch.object(
                EVIDENCE, "trusted_executable", side_effect=select_executable,
            ), patch.object(EVIDENCE.subprocess, "run", side_effect=select_java_home):
                with self.assertRaises(EVIDENCE.EvidenceError):
                    EVIDENCE.command_binding(parent_environment, broken_just, BUN_BINARY_SHA256, rust_proof, host_proof)
            self.assertFalse(install_marker.exists())

            self.assertFalse(install_marker.exists())

            commands_dir = Path(binding.path.split(os.pathsep)[0])
            (commands_dir / "grep").symlink_to(brew_bin / "grep")
            with self.assertRaisesRegex(EVIDENCE.EvidenceError, "authenticated artifact|directory changed"):
                EVIDENCE.assert_command_binding(binding, just)

    def test_descendant_tools_resolve_from_one_binding_and_ignore_parent_path(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            inputs = root / "krr-local-macos-inputs-fixture"
            trusted = inputs / "trusted-tools"
            trusted_commands = inputs / "trusted-commands"
            trusted_just = inputs / "trusted-just"
            hostile = root / "hostile"
            trusted.mkdir(parents=True)
            trusted_commands.mkdir(mode=0o700)
            trusted_just.mkdir()
            hostile.mkdir()
            names = (
                "just", "cargo", "rustc", "cargo-clippy", "clippy-driver", "rustfmt", "bun",
                "dot", "bash", "python3", "java", "git", "source_git", "sw_vers", "uname",
            )
            trusted_markers = root / "trusted-markers"
            hostile_markers = root / "hostile-markers"
            trusted_markers.mkdir()
            hostile_markers.mkdir()
            entries = {}
            for name in names:
                trusted_binary = trusted / name
                trusted_binary.write_text(
                    f"#!/bin/sh\nprintf trusted > '{trusted_markers / name}'\nexit 0\n", encoding="utf-8",
                )
                trusted_binary.chmod(0o755)
                hostile_binary = hostile / name
                hostile_binary.write_text(
                    f"#!/bin/sh\nprintf hostile > '{hostile_markers / name}'\nexit 0\n", encoding="utf-8",
                )
                hostile_binary.chmod(0o755)
                if name == "just":
                    just_binary = trusted_just / "just"
                    shutil.copyfile(trusted_binary, just_binary)
                    just_binary.chmod(0o755)
                    trusted_binary = just_binary
                entries[name] = (str(trusted_binary), EVIDENCE.sha256_bytes(trusted_binary.read_bytes()))
                if name != "source_git":
                    (trusted_commands / name).symlink_to(trusted_binary)
            just_path = Path(entries["just"][0])
            just = EVIDENCE.JustBinding(
                just_path, "just 1.40.0", "1.40.0", "just-1.40.0-aarch64-apple-darwin.tar.gz",
                "https://github.com/casey/just/releases/download/1.40.0/just-1.40.0-aarch64-apple-darwin.tar.gz",
                "a" * 64, entries["just"][1],
            )
            source_git = Path("/usr/bin/git").resolve(strict=True)
            entries["source_git"] = (str(source_git), EVIDENCE.sha256_bytes(source_git.read_bytes()))
            path = os.pathsep.join((str(trusted_commands), *EVIDENCE.SYSTEM_COMMAND_PATH))
            binding = EVIDENCE.CommandBinding(
                path, entries, {}, RUST_COMPONENT_DIGESTS, fixture_host_tool_proof(),
            )
            host_tools_patcher = patch.object(EVIDENCE, "verify_installed_host_tools")
            host_tools_patcher.start()
            self.addCleanup(host_tools_patcher.stop)
            EVIDENCE.assert_command_binding(binding, just)
            environment = {"PATH": path, "HOSTILE_PARENT_PATH": str(hostile)}
            result = subprocess.run(
                ("/bin/sh", "-c", "for tool in cargo rustc bun python3 bash java dot git; do \"$tool\" nested; done"),
                env=environment, text=True, capture_output=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual({path.name for path in trusted_markers.iterdir()}, {"cargo", "rustc", "bun", "python3", "bash", "java", "dot", "git"})
            self.assertFalse(any(hostile_markers.iterdir()))
            EVIDENCE.assert_command_binding(binding, just)

            injected = trusted_commands / "grep"
            injected.symlink_to(hostile / "grep")
            with self.assertRaisesRegex(EVIDENCE.EvidenceError, "directory changed"):
                EVIDENCE.assert_command_binding(binding, just)
            injected.unlink()
            (trusted_commands / "cargo").unlink()
            (trusted_commands / "cargo").symlink_to(trusted / "rustc")
            with self.assertRaisesRegex(EVIDENCE.EvidenceError, "alias changed"):
                EVIDENCE.assert_command_binding(binding, just)

    @unittest.skipUnless(os.name == "posix", "POSIX child shell fixture is required")
    def test_excluded_homebrew_shadows_cannot_turn_failing_child_check_into_success(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            commands = root / "trusted-commands"
            commands.mkdir(mode=0o700)
            homebrew = root / "opt-homebrew-bin"
            homebrew.mkdir()
            markers = root / "shadow-executed"
            for name in ("grep", "shasum"):
                shadow = homebrew / name
                shadow.write_text(
                    f"#!/bin/sh\nprintf '{name}\\n' >> '{markers}'\nexit 0\n", encoding="utf-8",
                )
                shadow.chmod(0o755)
            input_file = root / "input.txt"
            input_file.write_text("actual content\n", encoding="utf-8")
            environment = {"PATH": os.pathsep.join((str(commands), *EVIDENCE.SYSTEM_COMMAND_PATH))}
            result = subprocess.run(
                ("/bin/sh", "-c", f"grep required-token '{input_file}' && shasum -a 256 '{input_file}'"),
                env=environment, text=True, capture_output=True, check=False,
            )
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertFalse(markers.exists())


if __name__ == "__main__":
    unittest.main()
