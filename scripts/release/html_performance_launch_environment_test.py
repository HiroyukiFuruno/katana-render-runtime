import hashlib
import io
import os
from pathlib import Path
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from unittest.mock import patch

import html_performance_launch_environment as launch
import run_html_performance as runner
from run_html_performance_test import bootstrap_fixture

PRIVATE_FILES_SUPPORTED = bool(getattr(os, "O_NOFOLLOW", 0) and getattr(os, "O_DIRECTORY", 0))


class LaunchEnvironmentTest(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        (self.root / "tmp").mkdir()
        self.path = self.root / "tmp" / "launch.json"

    @unittest.skipUnless(PRIVATE_FILES_SUPPORTED, "private file operations unavailable")
    def test_roundtrip_binds_every_key_and_identity(self) -> None:
        seed = {"PATH": "/bin", "EXTRA": "first", "EMPTY": "", "UNICODE": "値"}
        digest = launch.capture(self.root, self.path, seed)
        restored = launch.replay(self.root, self.path, digest)
        self.assertEqual(restored, seed)
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)
        with patch.dict(os.environ, seed, clear=True):
            first = runner._child_environment("identity")
        with patch.dict(os.environ, restored, clear=True):
            second = runner._child_environment("identity")
        self.assertEqual(first, second)
        with self.assertRaises(FileExistsError):
            launch.capture(self.root, self.path, {"EXTRA": "changed"})
        self.assertEqual(launch.replay(self.root, self.path, digest), seed)

    @unittest.skipUnless(PRIVATE_FILES_SUPPORTED, "private file operations unavailable")
    def test_rejects_unsafe_paths_symlinks_and_permissions(self) -> None:
        with self.assertRaises(RuntimeError):
            launch.capture(self.root, self.root / "outside", {})
        with self.assertRaises(RuntimeError):
            launch.capture(self.root, self.root / "tmp" / ".." / "outside", {})
        digest = launch.capture(self.root, self.path, {"A": "B"})
        self.path.chmod(0o644)
        with self.assertRaises(RuntimeError):
            launch.replay(self.root, self.path, digest)
        link = self.root / "tmp" / "link"
        link.symlink_to(self.path)
        with self.assertRaises(OSError):
            launch.replay(self.root, link, digest)
        directory_link = self.root / "tmp" / "directory-link"
        directory_link.symlink_to(self.root / "tmp", target_is_directory=True)
        with self.assertRaises(OSError):
            launch.capture(self.root, directory_link / "new", {})

    @unittest.skipUnless(PRIVATE_FILES_SUPPORTED, "private file operations unavailable")
    def test_rejects_digest_and_malformed_documents(self) -> None:
        for content in (b'{}', b'{"version":1,"environment":{"A":1}}',
                        b'{"version":1,"environment":{"A":"B","A":"C"}}', b'bad'):
            self.path.write_bytes(content)
            self.path.chmod(0o600)
            with self.assertRaises(RuntimeError):
                launch.replay(self.root, self.path, hashlib.sha256(content).hexdigest())
            with self.assertRaises(RuntimeError):
                launch.replay(self.root, self.path, "0" * 64)
        with self.assertRaises(RuntimeError):
            launch.replay(self.root, self.path, "invalid")

    @unittest.skipUnless(hasattr(os, "mkfifo"), "FIFO unavailable")
    @unittest.skipUnless(PRIVATE_FILES_SUPPORTED, "private file operations unavailable")
    def test_replay_opens_fifo_nonblocking_and_rejects_it(self) -> None:
        os.mkfifo(self.path, 0o600)
        real_open = os.open

        def checked_open(path: str | bytes | os.PathLike[str], flags: int, mode: int = 0o777,
                         *, dir_fd: int | None = None) -> int:
            if path == self.path.name:
                self.assertTrue(flags & os.O_NONBLOCK)
            return real_open(path, flags, mode, dir_fd=dir_fd)

        with patch.object(os, "open", side_effect=checked_open):
            with self.assertRaisesRegex(RuntimeError, "not private"):
                launch.replay(self.root, self.path, "0" * 64)

    @unittest.skipUnless(PRIVATE_FILES_SUPPORTED, "private file operations unavailable")
    def test_bootstrap_replay_strips_flags_and_uses_seed(self) -> None:
        seed = {"PATH": "/bin", "EXTRA": "captured"}
        with bootstrap_fixture(environment={"PATH": "/bin", "EXTRA": "changed", "LIVE_ONLY": "value"}) as (_, run):
            (runner.ROOT / "tmp").mkdir()
            path = runner.ROOT / "tmp" / "launch.json"
            digest = launch.capture(runner.ROOT, path, seed)
            self.assertEqual(runner.bootstrap(("--replay-launch-environment", str(path),
                                               "--replay-launch-environment-sha256", digest,
                                               "produce", "--resume", "run")), 0)
            child = run.call_args_list[-1]
            self.assertEqual(child.args[0][-4:], ["--prepared", "produce", "--resume", "run"])
            self.assertEqual(child.kwargs["env"]["EXTRA"], "captured")
            self.assertNotIn("LIVE_ONLY", child.kwargs["env"])
            self.assertEqual(run.call_args_list[1].kwargs["env"], seed)

    @unittest.skipUnless(PRIVATE_FILES_SUPPORTED, "private file operations unavailable")
    def test_bootstrap_capture_strips_flags_before_prepared(self) -> None:
        with bootstrap_fixture(environment={"PATH": "/bin", "EXTRA": "captured"}) as (_, run):
            (runner.ROOT / "tmp").mkdir()
            path = runner.ROOT / "tmp" / "launch.json"
            stdout, stderr = io.StringIO(), io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                self.assertEqual(runner.bootstrap(("produce", "--capture-launch-environment", str(path),
                                                   "--resume", "run")), 0)
            self.assertEqual(stdout.getvalue(), "")
            self.assertIn("sha256=", stderr.getvalue())
            self.assertEqual(run.call_args_list[-1].args[0][-4:],
                             ["--prepared", "produce", "--resume", "run"])

    def test_replay_path_and_digest_are_required_together(self) -> None:
        for arguments in (("--replay-launch-environment", str(self.path), "produce"),
                          ("--replay-launch-environment-sha256", "0" * 64, "produce")):
            with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                runner.bootstrap(arguments)

    def test_unsupported_platform_rejects_private_files(self) -> None:
        with patch.object(os, "O_NOFOLLOW", 0, create=True):
            with self.assertRaisesRegex(RuntimeError, "unsupported"):
                launch.capture(self.root, self.path, {})
            with self.assertRaisesRegex(RuntimeError, "unsupported"):
                launch.replay(self.root, self.path, "0" * 64)

    def test_prepared_and_ordinary_dispatch_keep_producer_arguments(self) -> None:
        for prepared in (False, True):
            argv = ["runner", *(["--prepared"] if prepared else []), "produce", "--resume", "run"]
            with patch.object(sys, "argv", argv), patch.object(runner, "_verify_prepared_python") as verify:
                with patch.object(runner, "run_mode", return_value=0) as run:
                    self.assertEqual(runner.main(), 0)
                    run.assert_called_once_with("produce", ["--resume", "run"])
                    self.assertEqual(verify.call_count, int(prepared))


if __name__ == "__main__":
    unittest.main()
