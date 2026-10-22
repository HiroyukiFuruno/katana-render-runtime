from __future__ import annotations

import importlib.util
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/release/update_html_performance_harness_lock.py"
SPEC = importlib.util.spec_from_file_location("update_harness_lock", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise ImportError("harness lock updater unavailable")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class UpdateHtmlPerformanceHarnessLockTest(unittest.TestCase):
    @staticmethod
    def fixture() -> tuple[Path, Path]:
        root = Path(tempfile.mkdtemp())
        harness = root / MODULE.HARNESS_MANIFEST.parent
        harness.mkdir(parents=True)
        (harness / "Cargo.lock").write_bytes(b"lock\n")
        gate = root / MODULE.GATE
        gate.parent.mkdir(parents=True)
        gate.write_text('LOCK_SHA = "' + "0" * 64 + '"\n', encoding="utf-8")
        return root, gate

    def keep_fixture(self) -> tuple[Path, Path]:
        root, gate = self.fixture()
        self.addCleanup(shutil.rmtree, root)
        return root, gate

    def test_update_command_uses_harness_manifest_once(self) -> None:
        self.assertEqual(
            MODULE.update_command(["rtk", "cargo"]),
            ["rtk", "cargo", "update", "--manifest-path", str(MODULE.HARNESS_MANIFEST)],
        )

    def test_sync_lock_pin_updates_the_single_explicit_pin(self) -> None:
        root, gate = self.keep_fixture()
        MODULE.sync_lock_pin(root, "1" * 64)
        self.assertIn('LOCK_SHA = "' + "1" * 64 + '"', gate.read_text(encoding="utf-8"))

    def test_sync_lock_pin_rejects_missing_or_duplicate_pin(self) -> None:
        root, gate = self.keep_fixture()
        for source in ("pass\n", ('LOCK_SHA = "' + "0" * 64 + '"\n') * 2):
            gate.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "exactly one LOCK_SHA"):
                MODULE.sync_lock_pin(root, "1" * 64)

    def test_sync_lock_pin_rejects_non_sha256_digest(self) -> None:
        root, _gate = self.keep_fixture()
        for digest in ("", "f" * 63, "g" * 64):
            with self.subTest(digest=digest), self.assertRaisesRegex(ValueError, "lowercase SHA256"):
                MODULE.sync_lock_pin(root, digest)

    def test_parse_cargo_command_rejects_empty_nul_and_bad_quote(self) -> None:
        for value in ("", "cargo\0update", 'cargo "'):
            with self.subTest(value=value), self.assertRaises(ValueError):
                MODULE.parse_cargo_command(value)

    def test_run_update_passes_existing_cargo_wrapper_and_returns_lock_sha(self) -> None:
        root, gate = self.keep_fixture()
        runner = Mock(return_value=subprocess.CompletedProcess([], 0))
        digest = MODULE.run_update(["rtk", "cargo"], root, runner)
        self.assertEqual(runner.call_args.kwargs, {"cwd": root, "check": True})
        self.assertEqual(runner.call_args.args[0], MODULE.update_command(["rtk", "cargo"]))
        self.assertEqual(digest, MODULE.lock_sha(root))
        self.assertIn(digest, gate.read_text(encoding="utf-8"))

    def test_run_update_preserves_pin_when_cargo_fails(self) -> None:
        root, gate = self.keep_fixture()
        before = gate.read_text(encoding="utf-8")
        runner = Mock(side_effect=subprocess.CalledProcessError(17, ["cargo", "update"]))
        with self.assertRaises(subprocess.CalledProcessError):
            MODULE.run_update(["cargo"], root, runner)
        self.assertEqual(gate.read_text(encoding="utf-8"), before)

    def test_lock_and_gate_symlinks_are_rejected(self) -> None:
        root, gate = self.keep_fixture()
        lock = root / MODULE.HARNESS_MANIFEST.with_name("Cargo.lock")
        lock.unlink()
        lock.symlink_to(gate)
        with self.assertRaisesRegex(ValueError, "symlink"):
            MODULE.lock_sha(root)
        lock.unlink()
        lock.write_bytes(b"lock\n")
        gate.unlink()
        gate.symlink_to(lock)
        with self.assertRaisesRegex(ValueError, "symlink"):
            MODULE.sync_lock_pin(root, "1" * 64)


if __name__ == "__main__":
    unittest.main()
