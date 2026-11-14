from __future__ import annotations

import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

import cargo_capacity_guard as guard


class RootCoverageTestOnlyScopeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        self.scope = guard._root_coverage_commands()
        repository = Path(guard.__file__).resolve().parents[2]
        for relative in self.scope.ROOT_TEST_ONLY_SHA256:
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((repository / relative).read_bytes())
            destination.chmod(0o644)
        subprocess.run(["git", "init", "-q"], cwd=self.root, check=True)
        subprocess.run(["git", "add", "--", *self.scope.ROOT_TEST_ONLY_SHA256],
                       cwd=self.root, check=True)

    def validate_scope(self) -> None:
        guard._validate_root_test_only_scope(self.root, self.scope.ROOT_TEST_ONLY_SHA256)

    def test_fixed_sources_and_exact_regex(self) -> None:
        self.validate_scope()
        pattern = self.scope.root_coverage_ignore_regex(self.root)
        for relative in self.scope.ROOT_TEST_ONLY_SHA256:
            self.assertIsNotNone(re.fullmatch(pattern, str(self.root / relative)))
        for relative in (
            "crates/kdr-linter/src/bin/mathjax_consumer_facts/other_tests.rs",
            "crates/kdr-linter/src/bin/mathjax_consumer_facts/test_support.rs",
        ):
            self.assertIsNone(re.fullmatch(pattern, str(self.root / relative)))
        known = next(iter(self.scope.ROOT_TEST_ONLY_SHA256))
        self.assertIsNone(re.fullmatch(pattern, str(self.root.parent / "elsewhere" / known)))

    def test_changed_bytes_and_helper_are_rejected(self) -> None:
        relative = next(iter(self.scope.ROOT_TEST_ONLY_SHA256))
        path = self.root / relative
        path.write_bytes(path.read_bytes() + b"\nfn helper() {}\n")
        with self.assertRaisesRegex(RuntimeError, "fixed content"):
            self.validate_scope()

    def test_missing_file_is_rejected(self) -> None:
        relative = next(iter(self.scope.ROOT_TEST_ONLY_SHA256))
        (self.root / relative).unlink()
        with self.assertRaisesRegex(RuntimeError, "unavailable"):
            self.validate_scope()

    @unittest.skipIf(os.name == "nt", "Windows mode bits do not express POSIX permissions")
    def test_executable_file_mode_is_rejected(self) -> None:
        relative = next(iter(self.scope.ROOT_TEST_ONLY_SHA256))
        (self.root / relative).chmod(0o755)
        with self.assertRaisesRegex(RuntimeError, "fixed content"):
            self.validate_scope()

    def test_index_executable_mode_is_rejected(self) -> None:
        relative = next(iter(self.scope.ROOT_TEST_ONLY_SHA256))
        subprocess.run(["git", "update-index", "--chmod=+x", "--", relative],
                       cwd=self.root, check=True)
        with self.assertRaisesRegex(RuntimeError, "index mode"):
            self.validate_scope()

    @unittest.skipIf(os.name == "nt", "symlink creation is not universally available on Windows")
    def test_symlink_and_nonregular_files_are_rejected(self) -> None:
        relative = next(iter(self.scope.ROOT_TEST_ONLY_SHA256))
        path = self.root / relative
        outside = self.root / "outside"
        outside.write_bytes(path.read_bytes())
        path.unlink()
        path.symlink_to(outside)
        with self.assertRaisesRegex(RuntimeError, "escaped"):
            self.validate_scope()


if __name__ == "__main__":
    unittest.main()
