from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from typing import cast

import native_coverage_local_anchor as local_anchor
import native_coverage_result_test as fixture_helpers
import native_nextest_retry_checkpoint as checkpoint


RUN_ID = "f5b47296-b219-42a8-a7e9-4ec0ed80045f"
IDENTITY = "a" * 64


class NativeNextestRetryCheckpointTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name).resolve()
        fixture_helpers.initialize_fixture_repository(self.repo)
        self._write_artifacts()

    def _write_artifacts(self) -> None:
        run_directory = self.repo / "target/nextest-root-state" / RUN_ID
        run_directory.mkdir(parents=True)
        (run_directory / "store.zip").write_bytes(b"recorded-nextest-run")
        profile = self.repo / "target/llvm-cov-target/krr-coverage/profiles/root.profraw"
        profile.parent.mkdir(parents=True)
        profile.write_bytes(b"retained-profile")

    def _checkpoint_path(self) -> Path:
        return self.repo / checkpoint.CHECKPOINT_PATH

    def test_write_and_read_bind_real_artifact_tree_and_private_anchor(self) -> None:
        self.assertTrue(checkpoint.write_checkpoint(self.repo, IDENTITY, RUN_ID, 100))
        stored = checkpoint.read_checkpoint(self.repo, IDENTITY)
        self.assertIsNotNone(stored)
        value = cast(dict[str, object], stored)
        self.assertEqual(value["nextestRunId"], RUN_ID)
        self.assertEqual(value["testExit"], 100)
        self.assertEqual(set(value), checkpoint.CHECKPOINT_KEYS)
        anchor_path = (
            self.repo
            / ".git"
            / local_anchor.DIRECTORY_NAME
            / "root-nextest-retry.json"
        )
        receipt = self._checkpoint_path().read_bytes()
        payload = json.loads(receipt)
        self.assertTrue(
            local_anchor.verify(
                self.repo,
                "root-nextest-retry",
                IDENTITY,
                cast(str, payload["anchorEpoch"]),
                receipt,
            )
        )
        self.assertTrue(anchor_path.is_file())

    def test_rejects_identity_and_artifact_tree_changes(self) -> None:
        self.assertTrue(checkpoint.write_checkpoint(self.repo, IDENTITY, RUN_ID, 0))
        self.assertIsNone(checkpoint.read_checkpoint(self.repo, "b" * 64))
        profile = self.repo / "target/llvm-cov-target/krr-coverage/profiles/root.profraw"
        profile.write_bytes(b"changed-profile")
        self.assertIsNone(checkpoint.read_checkpoint(self.repo, IDENTITY))

    def test_inspection_reports_finite_rejection_reasons(self) -> None:
        self.assertEqual(
            checkpoint.inspect_checkpoint(self.repo, IDENTITY),
            (None, "missing-invalid-payload"),
        )
        self.assertTrue(checkpoint.write_checkpoint(self.repo, IDENTITY, RUN_ID, 0))
        stored, reason = checkpoint.inspect_checkpoint(self.repo, IDENTITY)
        self.assertEqual(reason, "accepted")
        self.assertIsNotNone(stored)
        self.assertEqual(
            checkpoint.inspect_checkpoint(self.repo, "b" * 64),
            (None, "identity-or-shape"),
        )

        self._checkpoint_path().write_bytes(b"not-json")
        self.assertEqual(
            checkpoint.inspect_checkpoint(self.repo, IDENTITY),
            (None, "missing-invalid-payload"),
        )
        self.assertTrue(checkpoint.write_checkpoint(self.repo, IDENTITY, RUN_ID, 0))
        anchor_path = (
            self.repo
            / ".git"
            / local_anchor.DIRECTORY_NAME
            / "root-nextest-retry.json"
        )
        anchor_value = json.loads(anchor_path.read_bytes())
        anchor_value["receiptSha256"] = "f" * 64
        anchor_path.write_text(json.dumps(anchor_value), encoding="utf-8")
        self.assertEqual(
            checkpoint.inspect_checkpoint(self.repo, IDENTITY), (None, "anchor")
        )

        self.assertTrue(checkpoint.write_checkpoint(self.repo, IDENTITY, RUN_ID, 0))
        profile = self.repo / "target/llvm-cov-target/krr-coverage/profiles/root.profraw"
        profile.write_bytes(b"changed-profile")
        self.assertEqual(
            checkpoint.inspect_checkpoint(self.repo, IDENTITY), (None, "artifacts")
        )

        checkpoint_path = self._checkpoint_path()
        checkpoint_path.unlink()
        checkpoint_path.symlink_to(profile)
        self.assertEqual(
            checkpoint.inspect_checkpoint(self.repo, IDENTITY),
            (None, "observation-error"),
        )

    def test_rejects_checkpoint_receipt_mutation(self) -> None:
        self.assertTrue(checkpoint.write_checkpoint(self.repo, IDENTITY, RUN_ID, 100))
        value = json.loads(self._checkpoint_path().read_bytes())
        value["schema"] = "unknown"
        self._checkpoint_path().write_text(json.dumps(value), encoding="utf-8")
        self.assertIsNone(checkpoint.read_checkpoint(self.repo, IDENTITY))

    def test_rejects_private_anchor_receipt_tampering(self) -> None:
        self.assertTrue(checkpoint.write_checkpoint(self.repo, IDENTITY, RUN_ID, 100))
        anchor_path = (
            self.repo
            / ".git"
            / local_anchor.DIRECTORY_NAME
            / "root-nextest-retry.json"
        )
        anchor_value = json.loads(anchor_path.read_bytes())
        anchor_value["receiptSha256"] = "f" * 64
        anchor_path.write_text(json.dumps(anchor_value), encoding="utf-8")
        self.assertIsNone(checkpoint.read_checkpoint(self.repo, IDENTITY))

    def test_missing_artifacts_and_noncanonical_exit_are_not_cacheable(self) -> None:
        self.assertFalse(checkpoint.write_checkpoint(self.repo, IDENTITY, RUN_ID, True))
        missing_run = "e5b47296-b219-42a8-a7e9-4ec0ed80045f"
        self.assertFalse(
            checkpoint.write_checkpoint(self.repo, IDENTITY, missing_run, 100)
        )
        self.assertIsNone(checkpoint.read_checkpoint(self.repo, IDENTITY))

    def test_invalidation_removes_cache_and_rotates_anchor(self) -> None:
        self.assertTrue(checkpoint.write_checkpoint(self.repo, IDENTITY, RUN_ID, 100))
        checkpoint.invalidate_checkpoint(self.repo, IDENTITY)
        self.assertFalse(self._checkpoint_path().exists())
        self.assertIsNone(checkpoint.read_checkpoint(self.repo, IDENTITY))


if __name__ == "__main__":
    unittest.main()
