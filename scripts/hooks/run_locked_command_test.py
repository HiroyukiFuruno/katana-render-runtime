from __future__ import annotations

import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from local_review_lock import review_lock
from local_review_state import ReviewError
from run_locked_command import (
    _LockContentionError,
    _acquire_lock,
    _cargo_clean_target,
    _main,
    run_locked_command,
)


class LockedCommandTest(unittest.TestCase):
    def _fake_rtk(self, root: Path, marker: Path) -> Path:
        executable = root / "bin/rtk"
        executable.parent.mkdir(parents=True)
        executable.write_text(
            f"#!{sys.executable}\n"
            "import sys\n"
            "from pathlib import Path\n"
            "if sys.argv[1:3] != ['proxy', 'cargo'] or sys.argv[3] != 'clean':\n"
            "    raise SystemExit(99)\n"
            f"Path({str(marker)!r}).write_text('ran', encoding='utf-8')\n",
            encoding="utf-8",
        )
        executable.chmod(0o755)
        return executable

    def _run_protected_clean(self, root: Path, rtk: Path, producer_lock: Path) -> int:
        return run_locked_command(
            ["--protect-root-producer", "--", str(rtk), "proxy", "cargo", "clean"],
            root / "outer/review.lock",
            producer_lock,
            root,
        )

    def _fake_performance_coverage_just(self, root: Path, marker: Path) -> Path:
        executable = root / "bin/just"
        executable.parent.mkdir(parents=True, exist_ok=True)
        hooks = str(Path(__file__).parent)
        producer_lock = root / "producer/root-nextest-producer.lock"
        executable.write_text(
            f"#!{sys.executable}\n"
            "import sys\n"
            "from pathlib import Path\n"
            "if sys.argv[1:] != ['performance-harness-coverage-run']:\n"
            "    raise SystemExit(99)\n"
            f"sys.path.insert(0, {hooks!r})\n"
            "from local_review_lock import review_lock\n"
            "from local_review_state import ReviewError\n"
            "try:\n"
            f"    with review_lock(Path({str(producer_lock)!r})):\n"
            "        pass\n"
            "except ReviewError:\n"
            "    pass\n"
            "else:\n"
            "    raise SystemExit(98)\n"
            f"Path({str(marker)!r}).write_text('ran', encoding='utf-8')\n",
            encoding="utf-8",
        )
        executable.chmod(0o755)
        return executable

    def _performance_coverage_fixture(
        self, root: Path
    ) -> tuple[Path, Path, Path, Path]:
        terminal_lock = root / ".moon/cache/krr-terminal-report.lock"
        producer_lock = root / "producer/root-nextest-producer.lock"
        marker = root / "child-ran"
        just = self._fake_performance_coverage_just(root, marker)
        terminal_lock.parent.mkdir(parents=True)
        producer_lock.parent.mkdir(parents=True)
        return terminal_lock, producer_lock, marker, just

    def _run_clean_with_refs(
        self,
        root: Path,
        executable: str,
        options: list[str],
        producer_lock: Path,
        protected_refs: list[Path],
        path: str | None = None,
    ) -> int:
        environment = {"CARGO_TARGET_DIR": str(root / "target")}
        if protected_refs:
            environment["CARGO_CAPACITY_PROTECTED_REFS"] = json.dumps(
                [str(reference) for reference in protected_refs]
            )
        if path is not None:
            environment["PATH"] = path
        with patch.dict(os.environ, environment):
            return run_locked_command(
                [
                    "--protect-root-producer",
                    "--",
                    executable,
                    "proxy",
                    "cargo",
                    "clean",
                    *options,
                ],
                root / "outer/review.lock",
                producer_lock,
                root,
            )

    def _planned_package_clean(self, root: Path, plan: list[Path], *, exit_code: int = 0,
                               options: list[str] | None = None) -> tuple[int, list[list[str]]]:
        observed: list[list[str]] = []
        def run(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            observed.append(command)
            if "--dry-run" in command:
                return subprocess.CompletedProcess(command, exit_code,
                                                   "".join(f"{path}\n" for path in plan), "")
            return subprocess.CompletedProcess(command, 0, "", "")
        selected = options if options is not None else [
            "--profile", "dev", "-p", "katana-render-runtime", "-p", "katana-render-runtime-assets",
        ]
        with patch.object(subprocess, "run", side_effect=run):
            status = self._run_clean_with_refs(root, "rtk", selected, root / "producer/lock",
                                               [root / "target/debug/krr"])
        return status, observed

    def test_disjoint_package_clean_uses_original_argv_after_dry_run(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            debug = root / "target/debug"
            debug.mkdir(parents=True)
            (debug / "krr").write_text("reserved", encoding="utf-8")
            artifact = debug / "package-artifact"
            artifact.write_text("package", encoding="utf-8")
            status, calls = self._planned_package_clean(root, [artifact])
            self.assertEqual(status, 0)
            self.assertEqual(len(calls), 2)
            self.assertIn("--dry-run", calls[0])
            self.assertIn("--verbose", calls[0])
            actual = [word for word in calls[0] if word not in ("--dry-run", "--verbose")]
            self.assertEqual(calls[1], actual)
            self.assertEqual(calls[1][-6:], ["--profile", "dev", "-p", "katana-render-runtime",
                                             "-p", "katana-render-runtime-assets"])

    def test_package_clean_rejects_unsafe_or_incomplete_plan(self) -> None:
        for case in ("reference", "ancestor", "hardlink", "outside", "failed", "empty",
                     "symlink", "missing", "relative"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                debug = root / "target/debug"
                debug.mkdir(parents=True)
                reference = debug / "krr"
                reference.write_text("reserved", encoding="utf-8")
                artifact = debug / "package-artifact"
                artifact.write_text("package", encoding="utf-8")
                selected = {"reference": reference, "ancestor": debug,
                            "outside": root / "outside", "failed": artifact,
                            "empty": artifact, "hardlink": debug / "hardlink",
                            "symlink": debug / "symlink", "missing": debug / "missing",
                            "relative": Path("relative-path")}[case]
                if case == "hardlink":
                    os.link(reference, selected)
                elif case == "symlink":
                    selected.symlink_to(artifact)
                elif case == "outside":
                    selected.write_text("outside", encoding="utf-8")
                status, calls = self._planned_package_clean(root, [] if case == "empty" else [selected],
                                                            exit_code=1 if case == "failed" else 0)
                self.assertEqual(status, 1)
                self.assertFalse(any("--dry-run" not in command for command in calls))
                self.assertEqual(reference.read_text(encoding="utf-8"), "reserved")

    def test_package_clean_rejects_unknown_package_without_running_clean(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            debug = root / "target/debug"
            debug.mkdir(parents=True)
            (debug / "krr").write_text("reserved", encoding="utf-8")
            artifact = debug / "artifact"
            artifact.write_text("package", encoding="utf-8")
            status, calls = self._planned_package_clean(root, [artifact], options=[
                "--profile", "dev", "-p", "unknown-package",
            ])
            self.assertEqual(status, 1)
            self.assertFalse(any("--dry-run" not in command for command in calls))

    def test_protected_clean_without_references_preserves_relative_target_dir(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "child-ran"
            rtk = self._fake_rtk(root, marker)
            with patch.dict(os.environ, {"PATH": str(rtk.parent), "CARGO_TARGET_DIR": "relative-target"}):
                os.environ.pop("CARGO_CAPACITY_PROTECTED_REFS", None)
                status = self._run_protected_clean(root, rtk, root / "producer/lock")
            self.assertEqual(status, 0)
            self.assertTrue(marker.exists())

    def test_protected_clean_rejects_target_reference_before_child(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            reserved = target / "debug/krr"
            reserved.parent.mkdir()
            reserved.write_text("binary", encoding="utf-8")
            marker = root / "child-ran"
            rtk = self._fake_rtk(root, marker)
            with patch.dict(os.environ, {
                "PATH": str(rtk.parent),
                "CARGO_TARGET_DIR": str(target),
                "CARGO_CAPACITY_PROTECTED_REFS": json.dumps([str(reserved)]),
            }):
                status = self._run_protected_clean(root, rtk, root / "producer/lock")
            self.assertEqual(status, 1)
            self.assertFalse(marker.exists())
            self.assertEqual(reserved.read_text(encoding="utf-8"), "binary")

    def test_protected_clean_allows_reference_outside_target(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            external = root / "reserved/krr"
            external.parent.mkdir()
            external.write_text("binary", encoding="utf-8")
            marker = root / "child-ran"
            rtk = self._fake_rtk(root, marker)
            with patch.dict(os.environ, {
                "PATH": str(rtk.parent),
                "CARGO_TARGET_DIR": str(target),
                "CARGO_CAPACITY_PROTECTED_REFS": json.dumps([str(external)]),
            }):
                status = self._run_protected_clean(root, rtk, root / "producer/lock")
            self.assertEqual(status, 0)
            self.assertTrue(marker.exists())
            self.assertEqual(external.read_text(encoding="utf-8"), "binary")

    def test_profile_dev_clean_rejects_reserved_debug_reference_before_child(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            reserved = target / "debug/krr"
            reserved.parent.mkdir(parents=True)
            reserved.write_text("binary", encoding="utf-8")
            marker = root / "child-ran"
            rtk = self._fake_rtk(root, marker)
            status = self._run_clean_with_refs(
                root,
                str(rtk),
                ["--profile", "dev"],
                root / "producer/lock",
                [reserved],
                str(rtk.parent),
            )
            self.assertEqual(status, 1)
            self.assertFalse(marker.exists())
            self.assertEqual(reserved.read_text(encoding="utf-8"), "binary")

    def test_profile_dev_exact_selectors_narrow_only_the_debug_directory(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            alternate = root / "alternate-target"
            with patch.dict(os.environ, {"CARGO_TARGET_DIR": str(target)}):
                for options in (["--profile", "dev"], ["--profile=dev"]):
                    command = ["--", "rtk", "proxy", "cargo", "clean", *options]
                    self.assertEqual(
                        _cargo_clean_target(command, root), (target / "debug").resolve()
                    )
                command = [
                    "--",
                    "rtk",
                    "proxy",
                    "cargo",
                    "clean",
                    "--target-dir",
                    str(alternate),
                    "--profile",
                    "dev",
                ]
                self.assertEqual(
                    _cargo_clean_target(command, root), (alternate / "debug").resolve()
                )
            linked_debug = target / "debug"
            linked_debug.symlink_to(root / "external-debug", target_is_directory=True)
            with patch.dict(os.environ, {"CARGO_TARGET_DIR": str(target)}):
                command = ["--", "rtk", "proxy", "cargo", "clean", "--profile", "dev"]
                self.assertEqual(_cargo_clean_target(command, root), target.resolve())

    def test_real_profile_dev_clean_preserves_reserved_coverage_reference(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "src").mkdir()
            (root / "Cargo.toml").write_text(
                '[package]\nname = "clean-scope-probe"\nversion = "0.1.0"\nedition = "2024"\n',
                encoding="utf-8",
            )
            (root / "src/main.rs").write_text("fn main() {}\n", encoding="utf-8")
            target = root / "target"
            debug_marker = target / "debug/probe"
            coverage_marker = target / "llvm-cov-target/krr-coverage/probe"
            debug_marker.parent.mkdir(parents=True)
            coverage_marker.parent.mkdir(parents=True)
            debug_marker.write_text("debug", encoding="utf-8")
            coverage_marker.write_text("coverage", encoding="utf-8")
            rtk = shutil.which("rtk")
            if rtk is None:
                self.fail("rtk executable missing for the real Cargo clean regression")
            status = self._run_clean_with_refs(
                root,
                rtk,
                ["--profile", "dev"],
                root / "producer/lock",
                [coverage_marker],
            )
            self.assertEqual(status, 0)
            self.assertFalse(debug_marker.exists())
            self.assertEqual(coverage_marker.read_text(encoding="utf-8"), "coverage")

    def test_profile_dev_unrecognized_selectors_keep_whole_target_scope(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            options = [
                ["--profile", "dev", "--doc"],
                ["--profile=dev", "--target", "aarch64-apple-darwin"],
                ["--profile", "dev", "-p", "probe"],
                ["--profile", "release"],
            ]
            with patch.dict(os.environ, {"CARGO_TARGET_DIR": str(target)}):
                for extra in options:
                    with self.subTest(extra=extra):
                        command = ["--", "rtk", "proxy", "cargo", "clean", *extra]
                        self.assertEqual(
                            _cargo_clean_target(command, root), target.resolve()
                        )

    def test_lock_is_held_for_child_and_released_after_success(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / "krr-terminal-report.lock"
            code = """from pathlib import Path
import sys
sys.path.insert(0, sys.argv[2])
from local_review_lock import review_lock
from local_review_state import ReviewError
try:
    with review_lock(Path(sys.argv[1])):
        raise SystemExit(9)
except ReviewError:
    pass
"""
            status = run_locked_command(
                ["--", sys.executable, "-c", code, str(lock), str(Path(__file__).parent)], lock
            )
            self.assertEqual(status, 0)
            with review_lock(lock):
                self.assertTrue(lock.exists())

    def test_parent_lock_rejects_child_until_release(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / "krr-terminal-report.lock"
            sentinel = Path(directory) / "child-ran"
            hooks = str(Path(__file__).parent)
            code = """import sys
from pathlib import Path
sys.path.insert(0, sys.argv[2])
from local_review_lock import ReviewError
from run_locked_command import run_locked_command
command = ["--", sys.executable, "-c", "from pathlib import Path; import sys; Path(sys.argv[1]).write_text('ran')", sys.argv[3]]
try:
    status = run_locked_command(command, Path(sys.argv[1]))
except ReviewError:
    raise SystemExit(0)
raise SystemExit(9 if status == 0 else 8)
"""
            with review_lock(lock):
                attempt = subprocess.run(
                    [sys.executable, "-c", code, str(lock), hooks, str(sentinel)],
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(attempt.returncode, 0, attempt.stderr)
                self.assertFalse(sentinel.exists())
            status = run_locked_command(
                ["--", sys.executable, "-c", "from pathlib import Path; import sys; Path(sys.argv[1]).write_text('ran')", str(sentinel)],
                lock,
            )
            self.assertEqual(status, 0)
            self.assertEqual(sentinel.read_text(encoding="utf-8"), "ran")

    def test_child_failure_status_is_preserved_and_lock_released(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / "krr-terminal-report.lock"
            status = run_locked_command(["--", sys.executable, "-c", "raise SystemExit(23)"], lock)
            self.assertEqual(status, 23)
            with review_lock(lock):
                self.assertTrue(lock.exists())

    def test_protected_child_holds_both_locks_until_exit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            outer_lock = root / "outer/review.lock"
            producer_lock = root / "producer/root-nextest-producer.lock"
            hooks = str(Path(__file__).parent)
            code = """import sys
from pathlib import Path
sys.path.insert(0, sys.argv[3])
from local_review_lock import review_lock
from local_review_state import ReviewError
for lock in (Path(sys.argv[1]), Path(sys.argv[2])):
    try:
        with review_lock(lock):
            pass
    except ReviewError:
        continue
    raise SystemExit(9)
raise SystemExit(0)
"""
            status = run_locked_command(
                ["--protect-root-producer", "--", sys.executable, "-c", code,
                 str(producer_lock), str(outer_lock), hooks],
                outer_lock,
                producer_lock,
            )
            self.assertEqual(status, 0)
            with review_lock(outer_lock), review_lock(producer_lock):
                self.assertTrue(outer_lock.exists())
                self.assertTrue(producer_lock.exists())

    def test_performance_coverage_mode_holds_only_root_lock(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            terminal_lock, producer_lock, marker, just = self._performance_coverage_fixture(root)

            with review_lock(terminal_lock), patch.dict(os.environ, {"PATH": str(just.parent)}):
                status = run_locked_command(
                    ["--performance-coverage"],
                    terminal_lock,
                    producer_lock,
                    root,
                )

            self.assertEqual(status, 0)
            self.assertEqual(marker.read_text(encoding="utf-8"), "ran")

    def test_performance_coverage_mode_rejects_busy_root_lock_and_extra_arguments(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            terminal_lock, producer_lock, marker, just = self._performance_coverage_fixture(root)

            with patch.dict(os.environ, {"PATH": str(just.parent)}):
                with review_lock(producer_lock):
                    with self.assertRaises(ReviewError):
                        run_locked_command(
                            ["--performance-coverage"],
                            terminal_lock,
                            producer_lock,
                            root,
                        )
                self.assertFalse(marker.exists())
                self.assertEqual(
                    run_locked_command(
                        ["--performance-coverage", "--", "arbitrary-command"],
                        terminal_lock,
                        producer_lock,
                        root,
                    ),
                    2,
                )
                self.assertFalse(marker.exists())

    def test_busy_root_producer_rejects_child_and_releases_outer_lock(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            outer_lock = root / "outer/review.lock"
            producer_lock = root / "producer/root-nextest-producer.lock"
            sentinel = root / "child-ran"
            hooks = str(Path(__file__).parent)
            producer_lock.parent.mkdir(parents=True)
            code = """import sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from local_review_state import ReviewError
from run_locked_command import run_locked_command
command = ["--protect-root-producer", "--", sys.executable, "-c",
           "from pathlib import Path; import sys; Path(sys.argv[1]).write_text('ran')", sys.argv[4]]
try:
    run_locked_command(command, Path(sys.argv[2]), Path(sys.argv[3]))
except ReviewError:
    raise SystemExit(0)
raise SystemExit(9)
"""
            with review_lock(producer_lock):
                attempt = subprocess.run(
                    [sys.executable, "-c", code, hooks, str(outer_lock),
                     str(producer_lock), str(sentinel)],
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(attempt.returncode, 0, attempt.stderr)
                self.assertFalse(sentinel.exists())
            with review_lock(outer_lock):
                self.assertTrue(outer_lock.exists())

    def test_child_launch_failure_releases_lock(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / "krr-terminal-report.lock"
            missing = Path(directory) / "missing-command"
            self.assertEqual(run_locked_command(["--", str(missing)], lock), 1)
            with review_lock(lock):
                self.assertTrue(lock.exists())

    def test_main_reports_lock_contention_without_traceback(self) -> None:
        for arguments in (["--", "child"], ["--performance-coverage"]):
            with self.subTest(arguments=arguments):
                stderr = io.StringIO()
                with patch(
                    "run_locked_command.run_locked_command",
                    side_effect=_LockContentionError("busy"),
                ), patch("run_locked_command.sys.stderr", stderr):
                    self.assertEqual(_main(arguments), 1)
                self.assertEqual(
                    stderr.getvalue(),
                    "another verification or update is active; retry after it finishes\n",
                )
                self.assertNotIn("Traceback", stderr.getvalue())

    def test_lock_body_review_error_is_not_reclassified(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            error = ReviewError("body failure")
            with self.assertRaises(ReviewError) as raised:
                with _acquire_lock(Path(directory) / "review.lock"):
                    raise error
            self.assertIs(raised.exception, error)

    def test_invalid_command_is_rejected_without_locking(self) -> None:
        self.assertEqual(run_locked_command([]), 2)
        self.assertEqual(run_locked_command(["--"]), 2)
        self.assertEqual(run_locked_command(["--protect-root-producer"]), 2)
        self.assertEqual(run_locked_command(["--protect-root-producer", "--"]), 2)
        self.assertEqual(run_locked_command(["--performance-coverage", "extra"]), 2)


if __name__ == "__main__":
    unittest.main()
