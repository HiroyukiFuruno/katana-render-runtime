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
            "macos": "15.0", "architecture": "arm64", "rustc": "rustc 1.90.0",
            "cargo": "cargo 1.90.0", "java": "openjdk 21", "bun": "1.4.2",
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
                patch.object(EVIDENCE, "tool_versions", return_value={"macos": "15", "architecture": "arm64"}),
                patch.object(EVIDENCE.Path, "home", return_value=Path(directory)),
                patch.object(EVIDENCE.subprocess, "run", return_value=subprocess.CompletedProcess([], 9, "", "failed")),
            ):
                with self.assertRaisesRegex(EVIDENCE.EvidenceError, "local command failed"):
                    EVIDENCE.collect(REPOSITORY, 7, publish=True)


if __name__ == "__main__":
    unittest.main()
