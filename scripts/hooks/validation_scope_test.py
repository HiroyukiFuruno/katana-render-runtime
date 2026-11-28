"""Exercise validation routing with an actual Git commit graph."""

from __future__ import annotations

from pathlib import Path
import subprocess
import tempfile
import unittest

from validation_scope import scope


class ValidationScopeTest(unittest.TestCase):
    def setUp(self) -> None:
        self.workspace = tempfile.TemporaryDirectory()
        self.addCleanup(self.workspace.cleanup)
        self.repo = Path(self.workspace.name)
        self.git("init", "--quiet")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "user.name", "Validation Scope Test")
        self.workflow = ".github/workflows/release.yml"
        self.write(self.workflow, "name: old\n")
        self.write("Cargo.toml", "[workspace]\n")
        self.base = self.commit()

    def git(self, *args: str) -> str:
        return subprocess.check_output(
            ["git", "-C", str(self.repo), *args], text=True,
        ).strip()

    def write(self, relative: str, content: str) -> None:
        path = self.repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def commit(self) -> str:
        self.git("add", "--all")
        self.git("commit", "--quiet", "-m", "test input")
        return self.git("rev-parse", "HEAD")

    def test_known_automation_diff_uses_scoped_checks(self) -> None:
        self.write(self.workflow, "name: new\n")
        self.assertEqual(scope(self.repo, self.base, self.commit()), "automation")

    def test_product_change_alongside_automation_requires_full_checks(self) -> None:
        self.write(self.workflow, "name: new\n")
        self.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n")
        self.assertEqual(scope(self.repo, self.base, self.commit()), "full")

    def test_unlisted_script_requires_full_checks(self) -> None:
        self.write("scripts/release/new.py", "print('test')\n")
        self.assertEqual(scope(self.repo, self.base, self.commit()), "full")

    def test_replacing_workflow_with_symlink_requires_full_checks(self) -> None:
        path = self.repo / self.workflow
        path.unlink()
        path.symlink_to("../../Cargo.toml")
        self.assertEqual(scope(self.repo, self.base, self.commit()), "full")

    def test_deletion_requires_full_checks(self) -> None:
        (self.repo / self.workflow).unlink()
        self.assertEqual(scope(self.repo, self.base, self.commit()), "full")

    def test_empty_diff_and_reversed_ancestry_require_full_checks(self) -> None:
        self.assertEqual(scope(self.repo, self.base, self.base), "full")
        self.write(self.workflow, "name: new\n")
        self.assertEqual(scope(self.repo, self.commit(), self.base), "full")

    def test_abbreviated_ref_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            scope(self.repo, self.base, self.base[:8])
