from __future__ import annotations

import importlib.util
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/release/update_html_performance_harness_lock.py"
SPEC = importlib.util.spec_from_file_location("update_harness_lock", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise ImportError("harness lock updater unavailable")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class UpdateHtmlPerformanceHarnessLockTest(unittest.TestCase):
    @staticmethod
    def retained_parent() -> bytes:
        registry = "registry+https://github.com/rust-lang/crates.io-index"
        return (
            'version = 4\n[[package]]\nname = "krr-html-performance-harness"\n'
            'version = "0.1.0"\ndependencies = ["utf8_iter", "crc32fast", "flate2", "smallvec"]\n'
            f'[[package]]\nname = "utf8_iter"\nversion = "1.0.4"\nsource = "{registry}"\n'
            f'checksum = "{"a" * 64}"\n'
            f'[[package]]\nname = "crc32fast"\nversion = "1.5.3"\nsource = "{registry}"\n'
            f'checksum = "{"b" * 64}"\n'
            f'[[package]]\nname = "flate2"\nversion = "1.1.10"\nsource = "{registry}"\n'
            f'checksum = "{"d" * 64}"\n'
            f'[[package]]\nname = "smallvec"\nversion = "1.16.4"\nsource = "{registry}"\n'
            f'checksum = "{"e" * 64}"\n'
        ).encode()

    @staticmethod
    def fixture() -> tuple[Path, Path]:
        root = Path(tempfile.mkdtemp())
        harness = root / MODULE.HARNESS_MANIFEST.parent
        harness.mkdir(parents=True)
        (harness / "Cargo.lock").write_bytes(b"lock\n")
        (root / "Cargo.lock").write_bytes(b"root lock\n")
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

    def test_retention_command_targets_the_requested_manifest_and_exact_version(self) -> None:
        package = MODULE.RetainedPackage("utf8_iter", "1.0.4", "reason")
        self.assertEqual(
            MODULE.retention_command(["rtk", "cargo"], MODULE.ROOT_MANIFEST, package),
            ["rtk", "cargo", "update", "--package", "utf8_iter", "--precise",
             MODULE.RETAINED_UTF8_ITER_VERSION,
             "--manifest-path", "Cargo.toml"],
        )
        self.assertEqual(
            MODULE.retention_command(["rtk", "cargo"], MODULE.HARNESS_MANIFEST, package),
            ["rtk", "cargo", "update", "--package", "utf8_iter", "--precise",
             MODULE.RETAINED_UTF8_ITER_VERSION,
             "--manifest-path", str(MODULE.HARNESS_MANIFEST)],
        )

    def test_retention_plan_is_bound_to_unique_parent_packages(self) -> None:
        parent = self.retained_parent()
        plan = MODULE.retention_plan(parent)
        self.assertEqual([(item.name, item.version) for item in plan],
                         [("utf8_iter", "1.0.4"), ("crc32fast", "1.5.3"),
                          ("flate2", "1.1.10"), ("smallvec", "1.16.4")])
        self.assertTrue(all(item.reason for item in plan))
        MODULE.verify_retained_packages(parent, (parent, parent), plan)
        changed = parent.replace(b'"1.5.3"', b'"1.5.4"')
        with self.assertRaisesRegex(ValueError, "retain immutable crc32fast"):
            MODULE.verify_retained_packages(parent, (parent, changed), plan)
        changed_smallvec = parent.replace(b'"1.16.4"', b'"1.16.5"')
        with self.assertRaisesRegex(ValueError, "retain immutable smallvec"):
            MODULE.verify_retained_packages(parent, (changed_smallvec, parent), plan)
        with self.assertRaises(ValueError):
            MODULE.retention_plan(parent.replace(b'"crc32fast"', b'"missing-crc"'))
        duplicate = parent + (
            '\n[[package]]\nname = "crc32fast"\nversion = "1.5.4"\n'
            'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
            f'checksum = "{"c" * 64}"\n'
        ).encode()
        with self.assertRaises(ValueError):
            MODULE.retention_plan(duplicate)

    def test_smallvec_retention_tracks_the_pinned_harness_url_closure(self) -> None:
        parent = MODULE.load_pinned_parent_lock(ROOT)
        _harness, _roots, closure = MODULE._harness_closure(parent)
        smallvec = next(identity for identity in closure if identity[0] == "smallvec")
        self.assertEqual(smallvec[1], "1.16.4")
        self.assertTrue(any(identity[0] == "idna" and smallvec in record[1]
                            for identity, record in closure.items()))
        self.assertIn(("smallvec", smallvec[1]),
                      [(item.name, item.version) for item in MODULE.retention_plan(parent)])

    def test_depends_update_runs_retention_updater_after_both_generic_lock_updates(self) -> None:
        justfile = (ROOT / "Justfile").read_text(encoding="utf-8")
        recipe = justfile.split("depends-update-mutations:\n", maxsplit=1)[1].split("\n\n", maxsplit=1)[0]
        lines = [line.strip() for line in recipe.splitlines() if line.strip()]
        root_update = lines.index("{{CARGO}} update")
        harness_update = lines.index(
            "python3 scripts/release/update_html_performance_harness_lock.py --cargo \"{{CARGO}}\""
        )
        self.assertLess(root_update, harness_update)
        self.assertEqual(sum("update_html_performance_harness_lock.py" in line for line in lines), 1)

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

    def test_run_update_retains_both_locks_before_syncing_the_harness_pin(self) -> None:
        root, gate = self.keep_fixture()
        runner = Mock(return_value=subprocess.CompletedProcess([], 0))
        parent = self.retained_parent()
        with patch.object(MODULE, "load_pinned_parent_lock", return_value=parent), \
             patch.object(MODULE, "verify_retained_packages") as retained, \
             patch.object(MODULE, "verify_utf8_iter_retention") as verify:
            digest = MODULE.run_update(["rtk", "cargo"], root, runner)
        plan = MODULE.retention_plan(parent)
        commands = [call.args[0] for call in runner.call_args_list]
        self.assertEqual(commands, [
            *[MODULE.retention_command(["rtk", "cargo"], MODULE.ROOT_MANIFEST, item) for item in plan],
            MODULE.update_command(["rtk", "cargo"]),
            *[MODULE.retention_command(["rtk", "cargo"], MODULE.HARNESS_MANIFEST, item) for item in plan],
        ])
        self.assertEqual(runner.call_args.kwargs, {"cwd": root, "check": True})
        verify.assert_called_once_with(
            (root / "Cargo.lock").read_bytes(),
            (root / MODULE.HARNESS_MANIFEST.with_name("Cargo.lock")).read_bytes(),
            parent,
        )
        retained.assert_called_once_with(parent, ((root / "Cargo.lock").read_bytes(),
                                         (root / MODULE.HARNESS_MANIFEST.with_name("Cargo.lock")).read_bytes()), plan)
        self.assertEqual(digest, MODULE.lock_sha(root))
        self.assertIn(digest, gate.read_text(encoding="utf-8"))

    def test_run_update_does_not_sync_pin_when_retention_validation_fails(self) -> None:
        root, gate = self.keep_fixture()
        before = gate.read_text(encoding="utf-8")
        runner = Mock(return_value=subprocess.CompletedProcess([], 0))
        with patch.object(MODULE, "load_pinned_parent_lock", return_value=self.retained_parent()), \
             patch.object(MODULE, "verify_retained_packages"):
            with patch.object(MODULE, "verify_utf8_iter_retention", side_effect=ValueError("closure differs")):
                with self.assertRaisesRegex(ValueError, "closure differs"):
                    MODULE.run_update(["rtk", "cargo"], root, runner)
        self.assertEqual(gate.read_text(encoding="utf-8"), before)

    def test_run_update_preserves_pin_when_cargo_fails(self) -> None:
        root, gate = self.keep_fixture()
        before = gate.read_text(encoding="utf-8")
        runner = Mock(side_effect=subprocess.CalledProcessError(17, ["cargo", "update"]))
        with patch.object(MODULE, "load_pinned_parent_lock", return_value=self.retained_parent()):
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
