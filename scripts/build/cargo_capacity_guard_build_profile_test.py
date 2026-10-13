import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

import cargo_capacity_guard as guard
from cargo_capacity_guard_active_profile_fixtures import (
    CompilerMessage,
    GuardReport,
    capacity_report,
    cargo_env,
    cargo_messages,
    create_workspace,
)


def build_call(
    root: Path, env: dict[str, str], release: bool = False
) -> tuple[subprocess.CompletedProcess[str], GuardReport | None]:
    env = dict(env, CARGO_CAPACITY_GUARD_ROOT=str(root))
    profile = "release" if release else "debug"
    cargo = shutil.which("cargo")
    if cargo is None:
        raise AssertionError("real Cargo is required")
    child = [cargo, "build", "--offline", "--message-format=json"]
    if release:
        child.append("--release")
    argv = [sys.executable, "-B", guard.__file__, "--retain-build-profile", profile,
            "--state", str(root / "state.json"), "--factor", "2", "--", *child]
    result = subprocess.run(argv, cwd=root, env=env, text=True, capture_output=True, timeout=60)
    report: GuardReport | None = None
    if result.returncode == 0:
        report_value: object = json.loads(result.stderr.splitlines()[0])
        report = capacity_report(report_value)
    return result, report


def artifacts(stdout: str) -> list[CompilerMessage]:
    return [row for row in cargo_messages(stdout) if row.get("reason") == "compiler-artifact"]


@unittest.skipIf(os.name == "nt", "POSIX allocated-byte fixture")
class BuildProfileTests(unittest.TestCase):
    def test_first_default_unsplit_build_and_warm_cache_need_no_created_profile(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            create_workspace(root)
            env = cargo_env(root / "target")
            env.pop("CARGO_TARGET_DIR")
            env.pop("CARGO_BUILD_BUILD_DIR")
            self.assertFalse((root / "target").exists())
            first, report = build_call(root, env)
            self.assertEqual(first.returncode, 0, first.stderr)
            if report is None:
                self.fail("successful build must produce a capacity report")
            self.assertEqual(report["current_bytes"], 0)
            self.assertEqual(report["actual_removed_bytes"], 0)
            second, report = build_call(root, env)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assertTrue(artifacts(second.stdout))
            self.assertTrue(all(row["fresh"] for row in artifacts(second.stdout)))
            meta = {"target_directory": str(root / "target"), "build_directory": str(root / "target")}
            self.assertEqual(guard.current_build_profiles(root / "target", meta, "debug"), (root / "target/debug",))

    def test_environment_split_and_configured_split_resolve_actual_existing_profiles(self) -> None:
        for configure in (False, True):
            with self.subTest(configure=configure), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                create_workspace(root)
                target = root / "target/configured"
                env = cargo_env(target, target / "build")
                if configure:
                    env.pop("CARGO_TARGET_DIR")
                    env.pop("CARGO_BUILD_BUILD_DIR")
                    (root / ".cargo").mkdir()
                    (root / ".cargo/config.toml").write_text(f'[build]\ntarget-dir="{target}"\nbuild-dir="{target / "build"}"\n')
                first, _ = build_call(root, env, True)
                self.assertEqual(first.returncode, 0, first.stderr)
                self.assertTrue((target / "release").is_dir())
                self.assertTrue((target / "build/release").is_dir())
                second, report = build_call(root, env, True)
                self.assertEqual(second.returncode, 0, second.stderr)
                if report is None:
                    self.fail("successful build must produce a capacity report")
                self.assertTrue(artifacts(second.stdout))
                self.assertTrue(all(row["fresh"] for row in artifacts(second.stdout)))
                self.assertEqual(report["actual_removed_bytes"], 0)

    def test_read_child_and_profile_directory_overrides_are_rejected_before_effects(self) -> None:
        bad = [("debug", ["cargo", "metadata"]), ("debug", ["cargo", "build", "--release"]),
               ("release", ["cargo", "build"]), ("debug", ["env", "cargo", "build"]),
               ("debug", ["cargo", "build", "--target-dir=elsewhere"])]
        for profile, command in bad:
            with self.subTest(command=command), self.assertRaises(RuntimeError):
                guard.validate_build_profile(profile, command)
        guard.validate_build_profile("debug", ["rtk", "cargo", "build", "--locked"])
        guard.validate_build_profile("release", ["cargo", "build", "--release"])


if __name__ == "__main__":
    unittest.main()
