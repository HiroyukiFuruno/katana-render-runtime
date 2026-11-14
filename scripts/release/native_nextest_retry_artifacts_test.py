from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from typing import cast

import native_nextest_retry_artifacts as artifacts

RUN_ID = "6ac3188d-70e1-4ff0-a2cf-08944b3350d0"
PROJECT = "_sfixture_project"
ARTIFACT_PATHS = (
    f"target/nextest-root-state/projects/{PROJECT}/records/runs/{RUN_ID}/store.zip",
    f"target/nextest-root-state/projects/{PROJECT}/records/runs/{RUN_ID}/run.log.zst",
    f"target/nextest-root-state/projects/{PROJECT}/records/runs/runs.json.zst",
    "target/llvm-cov-target/debug/deps/test-binary",
    "target/llvm-cov-target/krr-nextest-profraw.profraw",
)


class NativeNextestRetryArtifactsTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name).resolve()
        self.nextest = self.repo / "target/nextest-root-state"
        self.runs = self.nextest / "projects" / PROJECT / "records/runs"
        self.run_dir = self.runs / RUN_ID
        self.llvm = self.repo / "target/llvm-cov-target"
        self.write("target/nextest-root-state/projects/project-marker", b"project")
        self.write(
            f"target/nextest-root-state/projects/{PROJECT}/records/runs/runs.json.zst",
            b"index-v1",
        )
        self.write(
            f"target/nextest-root-state/projects/{PROJECT}/records/runs/{RUN_ID}/store.zip",
            b"store-v1",
        )
        self.write(
            f"target/nextest-root-state/projects/{PROJECT}/records/runs/{RUN_ID}/run.log.zst",
            b"record-v1",
        )
        self.write("target/llvm-cov-target/debug/deps/test-binary", b"binary-v1")
        self.write("target/llvm-cov-target/krr-nextest-profraw.profraw", b"profile-v1")

    def write(self, relative: str, contents: bytes) -> Path:
        path = self.repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(contents)
        return path

    def test_snapshot_binds_complete_trees_and_canonical_run(self) -> None:
        snapshot = artifacts.snapshot(self.repo, RUN_ID)
        self.assertEqual(snapshot["schema"], artifacts.SCHEMA)
        self.assertEqual(snapshot["runId"], RUN_ID)
        self.assertEqual(artifacts.recorded_run_ids(self.repo), {RUN_ID})
        self.assertTrue(artifacts.verify_snapshot(self.repo, snapshot))
        trees = cast(dict[str, dict[str, object]], snapshot["trees"])
        self.assertEqual(set(trees), {"nextest", "llvmCov"})
        self.assertGreater(cast(int, trees["nextest"]["entryCount"]), 0)
        self.assertGreater(cast(int, trees["llvmCov"]["totalBytes"]), 0)

    def test_any_record_index_binary_or_profile_change_invalidates_snapshot(
        self,
    ) -> None:
        for relative in ARTIFACT_PATHS:
            with self.subTest(relative=relative):
                original = self.repo / relative
                contents = original.read_bytes()
                recorded = artifacts.snapshot(self.repo, RUN_ID)
                original.write_bytes(contents + b"-changed")
                self.assertFalse(artifacts.verify_snapshot(self.repo, recorded))
                original.write_bytes(contents)

    def test_missing_record_index_binary_or_profile_invalidates_snapshot(self) -> None:
        for relative in ARTIFACT_PATHS:
            with self.subTest(relative=relative):
                recorded = artifacts.snapshot(self.repo, RUN_ID)
                (self.repo / relative).unlink()
                self.assertFalse(artifacts.verify_snapshot(self.repo, recorded))
                self.write(relative, b"restored")

    def test_invalid_run_ids_and_inexact_snapshot_schema_fail_closed(self) -> None:
        for run_id in ("6ac3188d", f"../{RUN_ID}", RUN_ID.upper()):
            with self.subTest(run_id=run_id), self.assertRaises(ValueError):
                artifacts.snapshot(self.repo, run_id)
        recorded = artifacts.snapshot(self.repo, RUN_ID)
        self.assertFalse(
            artifacts.verify_snapshot(self.repo, {**recorded, "extra": True})
        )
        self.assertFalse(
            artifacts.verify_snapshot(self.repo, {**recorded, "runId": "../escape"})
        )
        trees = cast(dict[str, dict[str, object]], recorded["trees"])
        bad_trees = {
            **trees,
            "nextest": {**trees["nextest"], "extra": True},
        }
        self.assertFalse(
            artifacts.verify_snapshot(self.repo, {**recorded, "trees": bad_trees})
        )
        bad_count = {
            **trees,
            "nextest": {**trees["nextest"], "entryCount": True},
        }
        self.assertFalse(
            artifacts.verify_snapshot(self.repo, {**recorded, "trees": bad_count})
        )

    def test_run_directory_requires_canonical_name_and_regular_store(self) -> None:
        noncanonical = self.runs / f"{{{RUN_ID}}}"
        noncanonical.mkdir()
        (noncanonical / "store.zip").write_bytes(b"ignored")
        no_store = self.runs / "97901d31-8c7f-4725-a42a-70b95cc538e7"
        no_store.mkdir()
        (no_store / "run.log.zst").write_bytes(b"record")
        self.assertEqual(artifacts.recorded_run_ids(self.repo), {RUN_ID})

    def test_symlinks_in_artifact_trees_are_rejected(self) -> None:
        outside = self.repo.parent / f"{self.repo.name}-outside"
        outside.mkdir()
        self.addCleanup(lambda: outside.rmdir())
        link = self.llvm / "debug/escape"
        link.symlink_to(outside, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "unsupported"):
            artifacts.snapshot(self.repo, RUN_ID)

    def test_symlinked_nextest_run_directory_is_rejected(self) -> None:
        run_link = self.runs / "97901d31-8c7f-4725-a42a-70b95cc538e7"
        run_link.symlink_to(self.run_dir, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            artifacts.recorded_run_ids(self.repo)

    def test_symlinked_repository_ancestor_is_rejected_before_resolution(self) -> None:
        alias = self.repo.parent / f"{self.repo.name}-alias"
        alias.symlink_to(self.repo, target_is_directory=True)
        self.addCleanup(alias.unlink)
        with self.assertRaisesRegex(ValueError, "symlink"):
            artifacts.snapshot(alias, RUN_ID)

    def test_missing_or_unsupported_artifact_tree_is_rejected(self) -> None:
        (self.llvm / "debug/deps/test-binary").unlink()
        (self.llvm / "krr-nextest-profraw.profraw").unlink()
        (self.llvm / "debug/deps").rmdir()
        (self.llvm / "debug").rmdir()
        self.llvm.rmdir()
        with self.assertRaisesRegex(ValueError, "unavailable"):
            artifacts.snapshot(self.repo, RUN_ID)
        self.llvm.mkdir()
        with self.assertRaisesRegex(ValueError, "unsupported"):
            artifacts.snapshot(self.repo, RUN_ID)
        self.assertFalse(artifacts.verify_snapshot(self.repo, {}))


if __name__ == "__main__":
    unittest.main()
