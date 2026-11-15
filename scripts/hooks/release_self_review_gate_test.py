from __future__ import annotations

import json
from pathlib import Path
from tempfile import TemporaryDirectory
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

    def test_reviewed_release_can_run_while_later_checks_are_pending(self) -> None:
        self.verify()
        self.assertEqual(self.git.call_count, 3)

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
