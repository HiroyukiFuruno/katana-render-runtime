import os
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Iterator
from contextlib import ExitStack, contextmanager
from importlib import metadata
from types import SimpleNamespace
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

import html_performance_python_identity as identity
import run_html_performance as runner


class DistributionStub:
    def __init__(self, root: Path, version: str, files: tuple[str, ...]) -> None:
        self.root = root
        self.version = version
        self.files = files

    def locate_file(self, relative: str) -> Path:
        return self.root / relative


@contextmanager
def bootstrap_fixture(
    state: str = "existing",
    environment: dict[str, str] | None = None,
    failure: Exception | None = None,
) -> Iterator[tuple[Path, MagicMock]]:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory).resolve()
        uv = root / "uv"
        uv.write_bytes(b"uv")
        venv = root / "stable-venv"
        if state == "existing":
            venv.mkdir()
            (venv / "pyvenv.cfg").write_text("existing", encoding="utf-8")
        elif state == "symlink":
            target = root / "target"
            target.mkdir()
            try:
                venv.symlink_to(target, target_is_directory=True)
            except OSError as error:
                raise unittest.SkipTest(f"directory symlinks unavailable: {error}") from error
        completed: subprocess.CompletedProcess[list[str]] = subprocess.CompletedProcess([], 0)
        with ExitStack() as stack:
            stack.enter_context(patch.object(runner, "ROOT", root))
            stack.enter_context(patch.object(runner, "VENV", venv))
            stack.enter_context(patch.object(shutil, "which", return_value=str(uv)))
            if environment is not None:
                stack.enter_context(patch.dict(os.environ, environment, clear=True))
            run = stack.enter_context(patch.object(subprocess, "run", return_value=completed, side_effect=failure))
            yield venv, run


class HtmlPerformanceRunnerTest(unittest.TestCase):
    def test_promotion_dispatch_preserves_arguments(self) -> None:
        arguments = ["--repo", "/repo", "--evidence-dir", "/repo/tmp/bundle",
                     "--promotion-evidence-dir", "/repo/tmp/proof", "--run-id", "run"]
        self.assertEqual(runner._command("promote", arguments), [
            sys.executable,
            str(runner.ROOT / "scripts/release/html_performance_promotion.py"),
            *arguments,
        ])

    def test_reuse_dispatch_preserves_arguments(self) -> None:
        arguments = ["--repo", "/repo", "--request-output", "/repo/tmp/request.json"]
        self.assertEqual(runner._command("reuse", arguments), [
            sys.executable,
            str(runner.ROOT / "scripts/release/html_performance_source_reuse_cli.py"),
            *arguments,
        ])

    def test_child_environment_binds_all_parent_values(self) -> None:
        original = {"PATH": "path", "VIRTUAL_ENV": "env", "EXTRA": "first"}
        with patch.dict(os.environ, original, clear=True):
            first = runner._child_environment("identity")
            os.environ["EXTRA"] = "second"
            second = runner._child_environment("identity")
        self.assertTrue(all(first[key] == value for key, value in original.items()))
        self.assertEqual(first[runner.IDENTITY_ENV], "identity")
        self.assertEqual(first[runner.VERSION_ENV], runner.SCIPY_VERSION)
        self.assertNotEqual(first[runner.ENVIRONMENT_ENV], second[runner.ENVIRONMENT_ENV])

    def test_bootstrap_reuses_venv_and_preserves_full_parent_environment(self) -> None:
        original = {"PATH": "original-path", "VIRTUAL_ENV": "original-env", "EXTRA": "bound"}
        with bootstrap_fixture(environment=original) as (venv, run):
            self.assertEqual(runner.bootstrap(("verify", "--acquire")), 0)
            self.assertEqual(run.call_count, 3)
            self.assertEqual(run.call_args_list[0].args[0][-1], "--check-base")
            sync = run.call_args_list[1].args[0]
            interpreter = venv / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
            self.assertEqual(sync[1:5], ["pip", "sync", "--python", str(interpreter)])
            self.assertIn("--require-hashes", sync)
            self.assertIn(str(runner.REQUIREMENTS), sync)
            child = run.call_args_list[2]
            self.assertEqual(child.args[0][-3:], ["--prepared", "verify", "--acquire"])
            environment = child.kwargs["env"]
            self.assertTrue(all(environment[key] == value for key, value in original.items()))
            self.assertIn(runner.BASE_PYTHON_ENV, environment)
            self.assertIn(runner.BASE_PYTHON_PATH_ENV, environment)
            self.assertIn(runner.UV_ENV, environment)
            self.assertIn(runner.UV_PATH_ENV, environment)

    def test_bootstrap_creates_missing_environment_once(self) -> None:
        with bootstrap_fixture(state="missing") as (_, run):
            self.assertEqual(runner.bootstrap(("test",)), 0)
            self.assertEqual(run.call_count, 4)
            self.assertEqual(run.call_args_list[0].args[0][1], "venv")
            self.assertEqual(run.call_args_list[1].args[0][-1], "--check-base")
            self.assertEqual(run.call_args_list[2].args[0][1:3], ["pip", "sync"])

    def test_bootstrap_rejects_symlink_environment(self) -> None:
        with bootstrap_fixture(state="symlink") as (_, run):
            with self.assertRaisesRegex(RuntimeError, "virtual environment path is invalid"):
                runner.bootstrap(("test",))
            run.assert_not_called()

    def test_prepared_python_rejects_wrong_base_identity(self) -> None:
        with patch.dict(os.environ, {runner.BASE_PYTHON_PATH_ENV: "/not-the-base"}, clear=True):
            with self.assertRaisesRegex(RuntimeError, "base Python identity differs"):
                runner._verify_prepared_python()

    def test_base_check_failure_prevents_package_sync(self) -> None:
        failure = subprocess.CalledProcessError(1, "base-check")
        with bootstrap_fixture(failure=failure) as (_, run):
            with self.assertRaises(subprocess.CalledProcessError):
                runner.bootstrap(("verify",))
            run.assert_called_once()
            self.assertEqual(run.call_args.args[0][-1], "--check-base")

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
            psutil = DistributionStub(root, "7.2.2", ("a.py", "b.py"))
            reflink = DistributionStub(root, "0.3.3", ("a.py", "b.py"))
            scipy = DistributionStub(root, "1.18.1", ("b.py", "a.py"))
            with patch.object(metadata, "distribution", side_effect=(numpy, psutil, reflink, scipy)):
                first = identity.installed_identity("1.18.1", "2.5.4")
            numpy.files = ("b.py", "a.py")
            scipy.files = ("a.py", "b.py")
            with patch.object(metadata, "distribution", side_effect=(numpy, psutil, reflink, scipy)):
                second = identity.installed_identity("1.18.1", "2.5.4")
            self.assertEqual(first, second)

    def test_identity_changes_when_installed_file_bytes_change(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "numpy.bin").write_bytes(b"before")
            (root / "psutil.bin").write_bytes(b"stable")
            (root / "scipy.bin").write_bytes(b"stable")
            (root / "reflink.bin").write_bytes(b"stable")
            numpy = DistributionStub(root, "2.5.4", ("numpy.bin",))
            psutil = DistributionStub(root, "7.2.2", ("psutil.bin",))
            reflink = DistributionStub(root, "0.3.3", ("reflink.bin",))
            scipy = DistributionStub(root, "1.18.1", ("scipy.bin",))
            with patch.object(metadata, "distribution", side_effect=(numpy, psutil, reflink, scipy)):
                before = identity.installed_identity("1.18.1", "2.5.4")
            (root / "numpy.bin").write_bytes(b"after")
            with patch.object(metadata, "distribution", side_effect=(numpy, psutil, reflink, scipy)):
                after = identity.installed_identity("1.18.1", "2.5.4")
            self.assertNotEqual(before, after)
            (root / "reflink.bin").write_bytes(b"changed")
            with patch.object(metadata, "distribution", side_effect=(numpy, psutil, reflink, scipy)):
                changed = identity.installed_identity("1.18.1", "2.5.4")
            self.assertNotEqual(after, changed)
            (root / "psutil.bin").write_bytes(b"changed")
            with patch.object(metadata, "distribution", side_effect=(numpy, psutil, reflink, scipy)):
                psutil_changed = identity.installed_identity("1.18.1", "2.5.4")
            self.assertNotEqual(changed, psutil_changed)

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
