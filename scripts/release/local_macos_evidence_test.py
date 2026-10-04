from __future__ import annotations

import copy
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


MODULE_PATH = Path(__file__).with_name("local_macos_evidence.py")
SPEC = importlib.util.spec_from_file_location("local_macos_evidence", MODULE_PATH)
assert SPEC and SPEC.loader
EVIDENCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(EVIDENCE)

REPOSITORY = "owner/repository"
BASE = "a" * 40
HEAD = "b" * 40
NOW = 1_800_000_000
STABLE_RUST = {
    "rustc": "1.99.0 (b940084d7 2026-09-28)",
    "cargo": "0.100.0 (5f94df478 2026-08-27)",
    "cargo_cli": "1.99.0 (5f94df478 2026-08-27)",
}


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
            "just": "just 1.40.0", "brew": "Homebrew 5.0.0", "graphviz": "dot - graphviz version 12",
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


def snapshots(comment: object, *, changed_second: bool = False, api_fail: bool = False):
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

    def test_accepts_current_owner_proof_bound_to_base_head_workflow_and_full_scope(self) -> None:
        self.assertTrue(EVIDENCE.verify_from_api(
            REPOSITORY, 7, expected_base=BASE, expected_head=HEAD,
            fetch=snapshots(valid_comment()), now=NOW, expected_workflow_digest="d" * 64,
        ))

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

    def test_rejects_java_other_than_21_and_bun_other_than_1_4_2(self) -> None:
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

        EVIDENCE.validate_pinned_tools({"java": 'java version "21.0.12"', "bun": "1.4.2"})
        for java, bun in (
            ('java version "17.0.13"', "1.4.2"),
            ('java version "22.0.1"', "1.4.2"),
            ("unparseable", "1.4.2"),
            ('java version "21.0.12"', "1.4.1"),
        ):
            with self.subTest(collection_java=java, collection_bun=bun):
                with self.assertRaises(EVIDENCE.EvidenceError):
                    EVIDENCE.validate_pinned_tools({"java": java, "bun": bun})

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
                '[build]\ntarget = "x86_64-apple-darwin"\nrustc = "alternate-rustc"\n'
                'rustc-wrapper = "compiler-wrapper"\nrustc-workspace-wrapper = "workspace-wrapper"\n'
                '[target.aarch64-apple-darwin]\nrunner = "/usr/bin/true"\n',
                encoding="utf-8",
            )
            with patch.dict(EVIDENCE.os.environ, {
                "CARGO": "true",
                "CARGO_BUILD_TARGET": "x86_64-apple-darwin",
                "RTK": "true",
                "RUSTFLAGS": "--target=x86_64-apple-darwin",
                "CARGO_ENCODED_RUSTFLAGS": "--target=x86_64-apple-darwin",
                "CARGO_BUILD_RUSTFLAGS": "--target=x86_64-apple-darwin",
                "CARGO_TARGET_DIR": "/tmp/cargo-target-cache",
            }, clear=True):
                environment = EVIDENCE.command_environment()
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
            self.assertEqual(environment["CARGO_TARGET_DIR"], "/tmp/cargo-target-cache")
            self.assertIn('target = "x86_64-apple-darwin"', (config_dir / "config.toml").read_text())
            self.assertIn('runner = "/usr/bin/true"', (config_dir / "config.toml").read_text())
            with patch.dict(EVIDENCE.os.environ, {"CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER": "cross-linker"}, clear=True):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "custom Rust compiler or target linker"):
                    EVIDENCE.command_environment()
            with patch.dict(EVIDENCE.os.environ, {"CARGO_BUILD_RUSTC_WRAPPER": "compiler-wrapper"}, clear=True):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "custom Rust compiler or target linker"):
                    EVIDENCE.command_environment()

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
        }
        EVIDENCE.validate_stable_rust_tools(current, STABLE_RUST)
        rejected = (
            {**current, "rustc": "rustc 1.91.0-nightly (nightly-hash 2025-09-14)"},
            {**current, "rustc": "rustc 1.91.0-beta (beta-hash 2025-09-14)"},
            {**current, "rustc": "rustc 1.89.0 (older-stable-hash 2025-08-01)"},
            {**current, "cargo": "cargo 1.98.0 (olderhash1 2026-08-20)"},
            {**current, "cargo": "cargo 0.100.0 (5f94df478 2026-08-27)"},
        )
        for tools in rejected:
            with self.subTest(tools=tools):
                with self.assertRaises(EVIDENCE.EvidenceError):
                    EVIDENCE.validate_stable_rust_tools(tools, STABLE_RUST)

    def test_parses_rustc_and_cargo_versions_from_official_channel_fixture(self) -> None:
        fixture = '''\
[pkg.rustc]
version = "1.99.0 (b940084d7 2026-09-28)"

[pkg.rustc.target.aarch64-apple-darwin]
available = true

[pkg.cargo]
version = "0.100.0 (5f94df478 2026-08-27)"
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

[pkg.cargo]
version = "0.100.0 (5f94df478 2026-08-27)"
'''
        parsed = EVIDENCE.parse_stable_manifest_versions(fixture)
        self.assertEqual(parsed["cargo_cli"], "1.99.0 (5f94df478 2026-08-27)")
        for cargo in (
            "cargo 0.100.0 (5f94df478 2026-08-27)",
            "cargo 1.98.0 (5f94df478 2026-08-27)",
            "cargo 1.99.0-nightly (5f94df478 2026-08-27)",
        ):
            with self.subTest(cargo=cargo), self.assertRaises(EVIDENCE.EvidenceError):
                EVIDENCE.validate_stable_rust_tools(
                    {"rustc": "rustc 1.99.0 (b940084d7 2026-09-28)", "cargo": cargo}, parsed
                )

    def test_manifest_fetch_failure_falls_back_to_hosted_ci(self) -> None:
        with patch.object(EVIDENCE, "fetch_stable_manifest_versions", side_effect=EVIDENCE.EvidenceError("offline")):
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
                patch.object(EVIDENCE, "api_request", side_effect=[
                    {"full_name": REPOSITORY, "owner": {"login": "owner"}}, {"login": "owner"},
                    {"number": 7, "state": "open", "base": {"sha": BASE, "ref": "master", "repo": {"full_name": REPOSITORY}},
                     "head": {"sha": HEAD, "repo": {"full_name": REPOSITORY}}},
                ]),
                patch.object(EVIDENCE, "macos_product_version", return_value="15.0"),
                patch.object(EVIDENCE, "require_standard_index_flags"),
                patch.object(EVIDENCE, "head_worktree_bytes_digest", return_value="source-hash"),
                patch.object(EVIDENCE, "tool_versions", return_value={"macos": "15", "architecture": "arm64"}),
                patch.object(EVIDENCE, "ROOT", Path(directory)),
                patch.object(EVIDENCE.subprocess, "run", return_value=subprocess.CompletedProcess([], 9, "", "failed")),
            ):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "local command failed"):
                    EVIDENCE.collect(REPOSITORY, 7, publish=True)

    def test_collector_rejects_wrong_java_before_quality_commands(self) -> None:
        versions = {
            "macos": "15.0", "architecture": "arm64", "rustc": "rustc stable",
            "cargo": "cargo stable", "java": 'openjdk version "17.0.13"',
            "bun": "1.4.2", "just": "just 1", "brew": "Homebrew 5", "graphviz": "dot 12",
        }
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(EVIDENCE.platform, "system", return_value="Darwin"),
                patch.object(EVIDENCE.platform, "machine", return_value="arm64"),
                patch.object(EVIDENCE, "git_status", return_value=""),
                patch.object(EVIDENCE, "run_git", side_effect=[HEAD, BASE]),
                patch.object(EVIDENCE, "workflow_digest", return_value="d" * 64),
                patch.object(EVIDENCE, "gh_token", return_value="existing-token"),
                patch.object(EVIDENCE, "api_request", side_effect=[
                    {"full_name": REPOSITORY, "owner": {"login": "owner"}}, {"login": "owner"},
                    {"number": 7, "state": "open", "base": {"sha": BASE, "ref": "master", "repo": {"full_name": REPOSITORY}},
                     "head": {"sha": HEAD, "repo": {"full_name": REPOSITORY}}},
                ]),
                patch.object(EVIDENCE, "macos_product_version", return_value="15.0"),
                patch.object(EVIDENCE, "require_standard_index_flags"),
                patch.object(EVIDENCE, "head_worktree_bytes_digest", return_value="source-hash"),
                patch.object(EVIDENCE, "tool_versions", return_value=versions),
                patch.object(EVIDENCE, "fetch_stable_manifest_versions", return_value=STABLE_RUST),
                patch.object(EVIDENCE, "ROOT", Path(directory)),
                patch.object(EVIDENCE.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "", "")) as run,
            ):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "Java 21"):
                    EVIDENCE.collect(REPOSITORY, 7, publish=True)
                self.assertEqual(run.call_count, len(EVIDENCE.PREPARATION_COMMANDS))

    def test_collector_rejects_nonstable_rust_before_quality_commands(self) -> None:
        versions = {
            "macos": "15.0", "architecture": "arm64",
            "rustc": "rustc 1.91.0-nightly (nightly-hash 2025-09-14)",
            "cargo": f"cargo {STABLE_RUST['cargo_cli']}", "java": 'openjdk version "21.0.12"',
            "bun": "1.4.2", "just": "just 1", "brew": "Homebrew 5", "graphviz": "dot 12",
        }
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(EVIDENCE.platform, "system", return_value="Darwin"),
                patch.object(EVIDENCE.platform, "machine", return_value="arm64"),
                patch.object(EVIDENCE, "git_status", return_value=""),
                patch.object(EVIDENCE, "run_git", side_effect=[HEAD, BASE]),
                patch.object(EVIDENCE, "workflow_digest", return_value="d" * 64),
                patch.object(EVIDENCE, "gh_token", return_value="existing-token"),
                patch.object(EVIDENCE, "api_request", side_effect=[
                    {"full_name": REPOSITORY, "owner": {"login": "owner"}}, {"login": "owner"},
                    {"number": 7, "state": "open", "base": {"sha": BASE, "ref": "master", "repo": {"full_name": REPOSITORY}},
                     "head": {"sha": HEAD, "repo": {"full_name": REPOSITORY}}},
                ]),
                patch.object(EVIDENCE, "macos_product_version", return_value="15.0"),
                patch.object(EVIDENCE, "require_standard_index_flags"),
                patch.object(EVIDENCE, "head_worktree_bytes_digest", return_value="source-hash"),
                patch.object(EVIDENCE, "tool_versions", return_value=versions),
                patch.object(EVIDENCE, "fetch_stable_manifest_versions", return_value=STABLE_RUST),
                patch.object(EVIDENCE, "ROOT", Path(directory)),
                patch.object(EVIDENCE.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "", "")) as run,
            ):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "current official stable channel"):
                    EVIDENCE.collect(REPOSITORY, 7, publish=True)
                self.assertEqual(run.call_count, len(EVIDENCE.PREPARATION_COMMANDS))


if __name__ == "__main__":
    unittest.main()
