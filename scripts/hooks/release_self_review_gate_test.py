from __future__ import annotations

import hashlib
import json
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import cast
import unittest
from unittest import mock

from local_review_state import ReviewError
from release_self_review_gate import verify_release_review


THREAD = "ffffffff-eeee-dddd-cccc-bbbbbbbbbbbb"
HEAD = "a" * 40


class ReleaseSelfReviewGateTest(unittest.TestCase):
    def setUp(self) -> None:
        directory = TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.home = Path(directory.name).resolve()
        self.root = self.home / "repository"
        self.root.mkdir()
        self.ledger = self.home / ".codex" / "hooks" / "task-state" / f"{THREAD}.json"
        self.ledger.parent.mkdir(parents=True)
        self.review: dict[str, object] = {
            "id": "pre-pr-self-review",
            "state": "done",
            "reviewed_head": HEAD,
            "pending_fixes": [],
        }
        self.document: dict[str, object] = {
            "session_id": THREAD,
            "cwd": str(self.root),
            "mode": "work",
            "tasks": [
                self.review,
                {"id": "release-ci", "state": "pending"},
                {"id": "full-quality-gate", "state": "pending"},
            ],
        }
        self.write_ledger()
        git = mock.patch("release_self_review_gate.command")
        self.git = git.start()
        self.addCleanup(git.stop)
        self.git.side_effect = ["release/v0.4.24\n", HEAD + "\n", ""]

    def write_ledger(self) -> None:
        self.ledger.write_text(json.dumps(self.document), encoding="utf-8")

    def verify(self, thread: str = THREAD) -> None:
        verify_release_review(self.root, self.home, thread)

    def add_minimum_reproduction(self, *, input_digest: str = "b" * 64,
                                 record_update: dict[str, object] | None = None,
                                 log: bytes = b"minimum reproduction\n") -> None:
        evidence = self.root / "tmp" / "minimum-reproduction"
        evidence.mkdir(parents=True, exist_ok=True)
        log_path = evidence / "stdout.log"
        log_path.write_bytes(log)
        record = {
            "exit_code": 0,
            "command": ["just", "automation-validation-timing-test"],
            "cwd": str(self.root),
            "input_digest": input_digest,
            "head_at_start": HEAD,
            "head_at_end": HEAD,
            "dirty_at_start": False,
        }
        if record_update:
            record.update(record_update)
        record_path = evidence / "exit.json"
        record_path.write_text(json.dumps(record), encoding="utf-8")
        self.review["heavy_failure"] = {
            "input_digest": input_digest,
            "minimum_reproduction": {
                "record": str(record_path.relative_to(self.root)),
                "log": str(log_path.relative_to(self.root)),
                "log_sha256": hashlib.sha256(log).hexdigest(),
            },
        }
        self.write_ledger()

    def test_reviewed_release_can_run_while_later_checks_are_pending(self) -> None:
        self.verify()
        self.assertEqual(self.git.call_count, 3)

    def test_full_quality_gate_requires_evidence_for_local_prerequisites(self) -> None:
        tasks = cast(list[object], self.document["tasks"])
        prerequisite: dict[str, object] = {
            "id": "workflow-reuse",
            "blocks_full_quality_gate": True,
            "evidence": "",
        }
        tasks.append(prerequisite)
        self.git.side_effect = None
        self.git.return_value = "release/v0.4.24\n"
        for state in ("pending", "running", "failed", "done"):
            with self.subTest(state=state):
                prerequisite["state"] = state
                self.write_ledger()
                with self.assertRaisesRegex(ReviewError, "ローカル作業が残っています.*workflow-reuse"):
                    self.verify()
        prerequisite["evidence"] = "verified reuse result"
        self.write_ledger()
        self.git.side_effect = ["release/v0.4.24\n", HEAD + "\n", ""]
        self.verify()

    def test_rejects_an_invalid_local_prerequisite_flag(self) -> None:
        tasks = cast(list[object], self.document["tasks"])
        tasks.append({"id": "workflow-reuse", "blocks_full_quality_gate": "true"})
        self.write_ledger()
        with self.assertRaisesRegex(ReviewError, "先行タスク指定が不正です"):
            self.verify()

    def test_no_codex_thread_does_not_add_requirements(self) -> None:
        self.verify("")
        self.git.assert_not_called()

    def test_non_release_branch_keeps_existing_quality_workflow(self) -> None:
        self.git.side_effect = ["fix/example\n"]
        self.verify()
        self.assertEqual(self.git.call_count, 1)

    def test_rejects_unfinished_review_and_declared_repairs(self) -> None:
        for update, message in (
            ({"state": "running"}, "自己レビューが完了していません"),
            ({"pending_fixes": ["handle a review finding"]}, "修正作業が残っています"),
            ({"pending_fixes": None}, "修正作業が残っています"),
        ):
            with self.subTest(update=update):
                original = self.review.copy()
                self.review.update(update)
                self.write_ledger()
                self.git.side_effect = ["release/v0.4.24\n"]
                with self.assertRaisesRegex(ReviewError, message):
                    self.verify()
                self.review.clear()
                self.review.update(original)

    def test_rejects_review_of_old_head_or_dirty_checkout(self) -> None:
        for actual_head, status, message in (
            ("b" * 40, "", "レビュー対象と一致しません"),
            (HEAD, " M src/lib.rs\n", "未コミットの変更があります"),
        ):
            with self.subTest(actual_head=actual_head, status=status):
                self.git.side_effect = ["release/v0.4.24\n", actual_head + "\n", status]
                with self.assertRaisesRegex(ReviewError, message):
                    self.verify()

    def test_heavy_failure_requires_current_clean_minimum_reproduction(self) -> None:
        self.add_minimum_reproduction()
        self.verify()

        cases: tuple[tuple[dict[str, object], str], ...] = (
            ({"exit_code": 1}, "現在のclean HEAD"),
            ({"exit_code": False}, "現在のclean HEAD"),
            ({"command": []}, "現在のclean HEAD"),
            ({"cwd": str(self.home)}, "現在のclean HEAD"),
            ({"input_digest": "c" * 64}, "現在のclean HEAD"),
            ({"head_at_start": "d" * 40}, "現在のclean HEAD"),
            ({"head_at_end": "e" * 40}, "現在のclean HEAD"),
            ({"dirty_at_start": True}, "現在のclean HEAD"),
        )
        for update, message in cases:
            with self.subTest(update=update):
                self.add_minimum_reproduction(record_update=update)
                self.git.side_effect = ["release/v0.4.24\n", HEAD + "\n", ""]
                with self.assertRaisesRegex(ReviewError, message):
                    self.verify()

    def test_heavy_failure_rejects_invalid_or_changed_evidence(self) -> None:
        self.add_minimum_reproduction()
        failure = cast(dict[str, object], self.review["heavy_failure"])
        reproduction = cast(dict[str, str], failure["minimum_reproduction"])
        log = self.root / reproduction["log"]
        log.write_text("changed\n", encoding="utf-8")
        with self.assertRaisesRegex(ReviewError, "ログhashが一致しません"):
            self.verify()

        self.review["heavy_failure"] = {"input_digest": "b" * 64}
        self.write_ledger()
        self.git.side_effect = ["release/v0.4.24\n", HEAD + "\n", ""]
        with self.assertRaisesRegex(ReviewError, "重い失敗の記録が不正です"):
            self.verify()

    def test_heavy_failure_rejects_linked_evidence_with_unchanged_content(self) -> None:
        self.add_minimum_reproduction()
        failure = cast(dict[str, object], self.review["heavy_failure"])
        reproduction = cast(dict[str, str], failure["minimum_reproduction"])
        log = self.root / reproduction["log"]
        original = log.rename(log.with_suffix(".original"))
        log.symlink_to(original)
        with self.assertRaisesRegex(ReviewError, "通常ファイルではありません"):
            self.verify()

    def test_rejects_missing_or_ambiguous_release_review(self) -> None:
        for tasks in ([], [self.review, self.review]):
            with self.subTest(tasks=tasks):
                self.document["tasks"] = tasks
                self.write_ledger()
                self.git.side_effect = ["release/v0.4.24\n"]
                with self.assertRaisesRegex(ReviewError, "自己レビュー項目が一意ではありません"):
                    self.verify()

    def test_rejects_unbound_ledger_and_invalid_thread_id(self) -> None:
        self.document["cwd"] = str(self.home / "another")
        self.write_ledger()
        with self.assertRaisesRegex(ReviewError, "台帳の作業場所またはセッションが一致しません"):
            self.verify()
        self.git.side_effect = ["release/v0.4.24\n"]
        with self.assertRaisesRegex(ReviewError, "CodexセッションIDが不正です"):
            self.verify("../another")

    def test_rejects_missing_ledger(self) -> None:
        self.ledger.unlink()
        with self.assertRaisesRegex(ReviewError, "作業台帳を読み込めません"):
            self.verify()
