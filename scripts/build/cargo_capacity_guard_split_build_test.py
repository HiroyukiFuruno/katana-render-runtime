import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from cargo_capacity_guard import size_bytes
from cargo_capacity_guard_lock import EvictionReport, ProfileLocks, _profiles, evict_artifacts


@unittest.skipIf(os.name == "nt", "POSIX native locking and allocated-byte fixtures")
class SplitBuildProfileTests(unittest.TestCase):
    def build_fixture(self, root: Path, split: bool) -> tuple[Path, Path, Path]:
        (root / "src").mkdir()
        (root / "Cargo.toml").write_text(
            '[package]\nname="split_fixture"\nversion="0.1.0"\nedition="2021"\n')
        (root / "src/lib.rs").write_text('pub fn value() -> usize { 42 }\n')
        target = root / "target"
        scope = target / "fixture-scope"
        env = {key: value for key, value in os.environ.items()
               if not key.startswith("CARGO_") and key not in {"RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"}}
        env.update(CARGO_TARGET_DIR=str(scope), CARGO_NET_OFFLINE="true", CARGO_BUILD_JOBS="2")
        env["CARGO_BUILD_BUILD_DIR"] = str(scope / "build" if split else scope)
        cargo = shutil.which("cargo")
        self.assertIsNotNone(cargo, "real Cargo is required")
        if cargo is None:
            self.fail("real Cargo is required")
        run = subprocess.run([cargo, "build", "--release", "--manifest-path", str(root / "Cargo.toml")],
                             cwd=root, env=env, text=True, capture_output=True, timeout=60)
        self.assertEqual(run.returncode, 0, run.stderr)
        profile = scope / "build/release" if split else scope / "release"
        self.assertTrue((profile / ".fingerprint").is_dir())
        self.assertTrue((profile / "deps").is_dir())
        return target, scope, profile

    def protected_refs(self, root: Path, target: Path) -> list[Path]:
        paths = []
        for i in range(10):
            path = (target / "consumers" if i < 6 else root / "frozen") / str(i)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((str(i) * 8192).encode())
            paths.append(path)
        return paths

    def identities(self, paths: list[Path]) -> list[tuple[str, int, int, str]]:
        return [(str(path), path.stat().st_dev, path.stat().st_ino,
                 hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]

    def finite_cleanup(
        self, target: Path, refs: list[Path], dry_run: bool = False
    ) -> tuple[int, EvictionReport]:
        before = size_bytes(target)
        if before is None:
            raise AssertionError("existing target size must be measurable")
        with ProfileLocks(target) as epoch:
            return evict_artifacts(target, before - 1, before, dry_run, refs, epoch)

    def test_real_cargo_normal_and_split_profiles_have_finite_candidates(self) -> None:
        for split in (False, True):
            with self.subTest(split=split), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                target, _, profile = self.build_fixture(root, split)
                refs = self.protected_refs(root, target)
                identities = self.identities(refs)
                self.assertIn(profile, _profiles(target))
                code, preview = self.finite_cleanup(target, refs, True)
                self.assertEqual(code, 0, preview)
                self.assertTrue(preview["candidate_paths"], preview)
                self.assertTrue(all(path.startswith(str(profile.relative_to(target)) + "/")
                                    for path in preview["candidate_paths"]))
                before = size_bytes(target)
                if before is None:
                    self.fail("existing target size must be measurable")
                code, actual = self.finite_cleanup(target, refs)
                self.assertEqual(code, 0, actual)
                self.assertGreater(actual["deleted_files"], 0)
                after = size_bytes(target)
                if after is None:
                    self.fail("existing target size must remain measurable")
                self.assertLess(after, before)
                self.assertEqual(self.identities(refs), identities)

    def assert_lock_busy(self, target: Path, lock: Path) -> None:
        script = ('import fcntl,sys; f=open(sys.argv[1],"a+"); '
                  'fcntl.flock(f,fcntl.LOCK_EX); print("locked",flush=True); sys.stdin.read()')
        process = subprocess.Popen([sys.executable, "-B", "-c", script, str(lock)],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        if process.stdin is None or process.stdout is None:
            process.kill()
            process.wait()
            self.fail("lock holder pipes are required")
        try:
            self.assertEqual(process.stdout.readline().strip(), "locked")
            with self.assertRaises(BlockingIOError), ProfileLocks(target):
                self.fail("busy real lock must prevent cleanup")
        finally:
            process.stdin.close()
            process.wait(timeout=10)
            process.stdout.close()
        self.assertEqual(process.returncode, 0)

    def test_real_split_output_and_build_locks_are_respected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target, scope, profile = self.build_fixture(Path(directory).resolve(), True)
            locks = [scope / "release/.cargo-lock", scope / "release/.cargo-artifact-lock"]
            self.assertTrue(all(lock.is_file() for lock in locks), "real split output locks required")
            locks.append(profile / ".cargo-build-lock")
            self.assertTrue(all(lock.is_file() for lock in locks))
            for lock in locks:
                with self.subTest(lock=lock):
                    self.assert_lock_busy(target, lock)

    def test_split_build_symlink_is_rejected_before_candidate_discovery(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target, scope, _ = self.build_fixture(root, True)
            outside = root / "saved-build"
            (scope / "build").rename(outside)
            (scope / "build").symlink_to(outside, target_is_directory=True)
            with self.assertRaisesRegex(RuntimeError, "kind or device boundary"):
                _profiles(target)
            self.assertTrue((outside / "release/.fingerprint").is_dir())

    def test_unknown_split_artifact_alias_is_retained(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target, _, profile = self.build_fixture(root, True)
            refs = self.protected_refs(root, target)
            candidates = [path for path in (profile / "deps").iterdir()
                          if path.is_file() and path.stat().st_nlink == 1 and path.suffix == ".d"]
            self.assertTrue(candidates, "real single-link Cargo dep-info required")
            artifact = candidates[0]
            alias = root / "external-alias"
            os.link(artifact, alias)
            identity = self.identities([artifact, alias, *refs])
            code, report = self.finite_cleanup(target, refs)
            self.assertEqual(code, 0, report)
            self.assertGreaterEqual(report["retained_unknown_alias_inodes"], 1)
            self.assertEqual(self.identities([artifact, alias, *refs]), identity)


if __name__ == "__main__":
    unittest.main()
