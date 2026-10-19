from __future__ import annotations

import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import uuid

import native_nextest_coverage_commands as commands
import native_nextest_retry_artifacts as artifacts
import native_nextest_retry_checkpoint as checkpoint
import native_nextest_root_coverage as root_coverage


class RootCoverageCommandTests(unittest.TestCase):
    def test_full_and_retry_keep_the_same_complete_scope(self) -> None:
        repo = Path("/krr")
        parent = str(uuid.uuid4())
        full = commands.nextest_arguments(repo, "2", None)
        retry = commands.nextest_arguments(repo, "2", parent)
        self.assertEqual(retry, [*full, "-R", parent])
        for required in (
            "--workspace", "--all-targets", "--all-features", "--locked",
            "--no-report", "--no-fail-fast",
        ):
            self.assertIn(required, full)
        self.assertEqual(full[full.index("--cargo-profile") + 1], "krr-coverage")
        self.assertEqual(full[full.index("--retries") + 1], "0")
        self.assertEqual(full[-1], "--test-threads=2")
        self.assertNotIn("--no-clean", full)
        self.assertFalse(any(
            argument.startswith("--test-threads=")
            for argument in commands.nextest_arguments(repo, "", None)
        ))

    def test_retry_never_resolves_a_mutable_latest_or_prefix(self) -> None:
        for parent in ("latest", "12345678", "../record", str(uuid.uuid4()).upper()):
            with self.subTest(parent=parent), self.assertRaises(ValueError):
                commands.nextest_arguments(Path("/krr"), "2", parent)
        for threads in ("0", "-1", "2.5", "two"):
            with self.subTest(threads=threads), self.assertRaises(ValueError):
                commands.nextest_arguments(Path("/krr"), threads, None)

    def test_report_is_bound_to_the_fixed_target_output(self) -> None:
        self.assertEqual(commands.clean_arguments(), [
            "llvm-cov", "clean", "--workspace", "--profraw-only",
        ])
        arguments = commands.report_arguments(Path("/krr"))
        self.assertEqual(arguments, [
            "llvm-cov", "report", "--profile", "krr-coverage", "--locked", "--json",
            "--summary-only", "--output-path",
            "/krr/tmp/quality/standalone-performance-coverage/root.json",
        ])


class RootCoverageExecutionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="krr-root-retry-unit-")
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name).resolve()
        (self.repo / commands.REPORT_DIRECTORY).mkdir(parents=True)

    def test_attempts_and_process_evidence_are_distinct_and_exact(self) -> None:
        first = root_coverage._attempt_directory(self.repo)
        second = root_coverage._attempt_directory(self.repo)
        self.assertNotEqual(first, second)
        argv = [sys.executable, "-c", (
            "import sys;print('stdout-evidence');"
            "print('stderr-evidence',file=sys.stderr);sys.exit(100)"
        )]
        code = root_coverage._execute(first, "actual-process", argv, self.repo, dict(os.environ))
        self.assertEqual(code, 100)
        logs = first / "actual-process"
        self.assertEqual(json.loads((logs / "argv.json").read_text()), argv)
        self.assertEqual((logs / "stdout.raw").read_bytes(), b"stdout-evidence\n")
        self.assertEqual((logs / "stderr.raw").read_bytes(), b"stderr-evidence\n")
        self.assertEqual(json.loads((logs / "result.json").read_text())["exit"], 100)
        self.assertGreater(json.loads((logs / "pid.json").read_text())["pid"], 0)
        with self.assertRaises(FileExistsError):
            root_coverage._execute(first, "actual-process", argv, self.repo, dict(os.environ))

    def test_successful_checkpoint_uses_report_only_without_a_test_process(self) -> None:
        run_id = str(uuid.uuid4())
        directory = root_coverage._attempt_directory(self.repo)
        result = root_coverage._test(
            self.repo, ["must-not-execute"], "2", "a" * 64,
            {"testExit": 0, "nextestRunId": run_id}, directory,
        )
        self.assertEqual(result, (0, run_id))
        self.assertEqual(list(directory.iterdir()), [])

    def test_root_producers_cannot_collect_concurrently(self) -> None:
        lock = self.repo / "root-producer.lock"
        import_paths = list(sys.path)
        with root_coverage._root_lock(lock):
            with self.assertRaisesRegex(ValueError, "another local review is active"):
                with root_coverage._root_lock(lock):
                    self.fail("a second root producer acquired the active lock")
        with root_coverage._root_lock(lock):
            self.assertTrue(lock.is_file())
        self.assertEqual(sys.path, import_paths)

    def test_full_collection_cleans_first_and_retry_keeps_prior_profiles(self) -> None:
        run_id = str(uuid.uuid4())
        cases: tuple[tuple[dict[str, object] | None, list[int], list[str], int, bool], ...] = (
            (None, [0, 0], ["clean", "tests"], 0, True),
            ({"testExit": 100, "nextestRunId": run_id}, [0], ["tests"], 0, True),
            (None, [23], ["clean"], 23, True),
            (None, [0, 0], ["clean", "tests"], 0, False),
            ({"testExit": 0, "nextestRunId": run_id}, [0, 0], ["clean", "tests"], 0, False),
        )
        for cached, exits, stages, expected_exit, cacheable in cases:
            with self.subTest(stages=stages, expected_exit=expected_exit, cacheable=cacheable):
                directory = root_coverage._attempt_directory(self.repo)
                with patch.object(root_coverage, "_execute", side_effect=exits) as execute, \
                        patch.object(checkpoint, "invalidate_checkpoint"), \
                        patch.object(artifacts, "recorded_run_ids", side_effect=ValueError) as records, \
                        patch.object(root_coverage, "_new_run_id", return_value=run_id) as select_run:
                    code, _ = root_coverage._test(
                        self.repo, ["cargo"], "2", "a" * 64, cached, directory, cacheable,
                    )
                self.assertEqual(code, expected_exit)
                self.assertEqual([call.args[1] for call in execute.call_args_list], stages)
                if not cacheable:
                    records.assert_not_called()
                    select_run.assert_not_called()
                for call in execute.call_args_list:
                    argv = call.args[2]
                    if call.args[1] == "clean":
                        self.assertEqual(argv[-5:], ["cargo", *commands.clean_arguments()])
                    else:
                        self.assertIn("--no-report", argv)
                        self.assertNotIn("--no-clean", argv)
                        self.assertEqual("-R" in argv, cached is not None and cacheable)

    def test_new_record_selection_requires_exactly_one_complete_record(self) -> None:
        first = self._record()
        self.assertEqual(root_coverage._new_run_id(self.repo, set()), first)
        second = self._record()
        self.assertIsNone(root_coverage._new_run_id(self.repo, set()))
        self.assertEqual(root_coverage._new_run_id(self.repo, {first}), second)

    def test_collection_and_profile_only_clean_keep_the_current_compiled_profile(self) -> None:
        profile = self.repo / "target/llvm-cov-target/krr-coverage"
        profile.mkdir(parents=True)
        plans = [
            ("--coverage-collection-domain", commands.report_arguments(self.repo)),
            ("--coverage-domain", commands.clean_arguments()),
        ]
        for mode, arguments in plans:
            with self.subTest(mode=mode):
                argv = root_coverage._guarded(self.repo, mode, ["rtk", "cargo"], arguments)
                self.assertEqual(argv[argv.index("--retain-profile") + 1], str(profile))
                self.assertEqual(argv[argv.index("--") + 1:], ["rtk", "cargo", *arguments])
        environment = root_coverage._child_environment(self.repo)
        self.assertEqual(environment["CARGO_LLVM_COV_TARGET_DIR"], str(profile.parent))
        self.assertEqual(environment["NEXTEST_STATE_DIR"], str(self.repo / "target/nextest-root-state"))
        self.assertNotIn("LLVM_PROFILE_FILE_NAME", environment)
        self.assertNotIn("LLVM_PROFILE_FILE", environment)

    def _record(self) -> str:
        run_id = str(uuid.uuid4())
        directory = self.repo / "target/nextest-root-state/projects/unit/records/runs" / run_id
        directory.mkdir(parents=True)
        (directory / "store.zip").write_bytes(b"artifact-unit-only")
        return run_id


if __name__ == "__main__":
    unittest.main()
