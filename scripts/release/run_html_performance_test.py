import subprocess
import tempfile
from importlib import metadata
from types import SimpleNamespace
import unittest
from pathlib import Path
from unittest.mock import patch

import html_performance_python_identity as identity
import run_html_performance as runner


class DistributionStub:
    def __init__(self, root: Path, version: str, files: tuple[str, ...]) -> None:
        self.root = root
        self.version = version
        self.files = files

    def locate_file(self, relative: str) -> Path:
        return self.root / relative


class HtmlPerformanceRunnerTest(unittest.TestCase):
    def test_changed_library_identity_fails_after_child(self) -> None:
        completed: subprocess.CompletedProcess[list[str]] = subprocess.CompletedProcess([], 0)
        with patch.object(runner, "installed_identity", side_effect=("before", "after")), patch.object(subprocess, "run", return_value=completed):
            self.assertEqual(runner.run_mode("test", ()), 1)

    def test_child_failure_is_preserved_when_identity_is_stable(self) -> None:
        completed: subprocess.CompletedProcess[list[str]] = subprocess.CompletedProcess([], 37)
        with patch.object(runner, "installed_identity", return_value="same"), patch.object(subprocess, "run", return_value=completed):
            self.assertEqual(runner.run_mode("test", ()), 37)

    def test_child_failure_is_preserved_when_identity_changes(self) -> None:
        completed: subprocess.CompletedProcess[list[str]] = subprocess.CompletedProcess([], 37)
        with patch.object(runner, "installed_identity", side_effect=("before", "after")), patch.object(subprocess, "run", return_value=completed):
            self.assertEqual(runner.run_mode("test", ()), 37)

    def test_identity_is_stable_when_inventory_order_changes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, content in (("a.py", b"a"), ("b.py", b"b")):
                (root / name).write_bytes(content)
            numpy = DistributionStub(root, "2.5.4", ("a.py", "b.py"))
            reflink = DistributionStub(root, "0.3.3", ("a.py", "b.py"))
            scipy = DistributionStub(root, "1.18.1", ("b.py", "a.py"))
            with patch.object(metadata, "distribution", side_effect=(numpy, reflink, scipy)):
                first = identity.installed_identity("1.18.1", "2.5.4")
            numpy.files = ("b.py", "a.py")
            scipy.files = ("a.py", "b.py")
            with patch.object(metadata, "distribution", side_effect=(numpy, reflink, scipy)):
                second = identity.installed_identity("1.18.1", "2.5.4")
            self.assertEqual(first, second)

    def test_identity_changes_when_installed_file_bytes_change(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "numpy.bin").write_bytes(b"before")
            (root / "scipy.bin").write_bytes(b"stable")
            (root / "reflink.bin").write_bytes(b"stable")
            numpy = DistributionStub(root, "2.5.4", ("numpy.bin",))
            reflink = DistributionStub(root, "0.3.3", ("reflink.bin",))
            scipy = DistributionStub(root, "1.18.1", ("scipy.bin",))
            with patch.object(metadata, "distribution", side_effect=(numpy, reflink, scipy)):
                before = identity.installed_identity("1.18.1", "2.5.4")
            (root / "numpy.bin").write_bytes(b"after")
            with patch.object(metadata, "distribution", side_effect=(numpy, reflink, scipy)):
                after = identity.installed_identity("1.18.1", "2.5.4")
            self.assertNotEqual(before, after)
            (root / "reflink.bin").write_bytes(b"changed")
            with patch.object(metadata, "distribution", side_effect=(numpy, reflink, scipy)):
                changed = identity.installed_identity("1.18.1", "2.5.4")
            self.assertNotEqual(after, changed)

    def test_identity_rejects_missing_inventory_file_and_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            missing_inventory = SimpleNamespace(version="1.18.1", files=None)
            with patch.object(metadata, "distribution", return_value=missing_inventory):
                with self.assertRaisesRegex(RuntimeError, "no installed file inventory"):
                    identity._distribution_identity("scipy", "1.18.1")

            missing_file = DistributionStub(root, "1.18.1", ("missing",))
            with patch.object(metadata, "distribution", return_value=missing_file):
                with self.assertRaisesRegex(RuntimeError, "installed file is unavailable"):
                    identity._distribution_identity("scipy", "1.18.1")

            (root / "present").write_bytes(b"present")
            (root / "link").symlink_to(root / "present")
            symlink = DistributionStub(root, "1.18.1", ("link",))
            with patch.object(metadata, "distribution", return_value=symlink):
                with self.assertRaisesRegex(RuntimeError, "installed file is unavailable"):
                    identity._distribution_identity("scipy", "1.18.1")

    def test_version_mismatch_is_rejected(self) -> None:
        distribution = SimpleNamespace(version="1.0.0", files=())
        with patch.object(metadata, "distribution", return_value=distribution):
            with self.assertRaisesRegex(RuntimeError, "version differs"):
                identity._distribution_identity("scipy", "1.18.1")
            with self.assertRaisesRegex(RuntimeError, "version differs"):
                identity._distribution_identity("reflink-copy", "0.3.3")
