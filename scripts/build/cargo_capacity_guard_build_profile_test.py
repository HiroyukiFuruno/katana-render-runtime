import json
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from unittest.mock import patch

import cargo_capacity_guard as guard
from cargo_capacity_guard_build_script_ownership import source_pin
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


class CheckDomainTests(unittest.TestCase):
    @staticmethod
    def command(domain: str, cargo: str = "cargo") -> list[str]:
        command = [cargo, "clippy", "-j", "2"]
        if domain == "root":
            command += ["--workspace", "--all-targets", "--all-features"]
        else:
            command += ["--manifest-path", "scripts/html/performance-harness/Cargo.toml",
                        "--all-targets", "--no-default-features", "--features",
                        domain.removeprefix("performance-"), "--locked"]
        return [*command, "--", *guard.CLIPPY_DENY_FLAGS]

    def test_only_three_fixed_clippy_recipes_are_admitted(self) -> None:
        for domain in guard.CHECK_DOMAINS:
            with self.subTest(domain=domain):
                self.assertEqual(guard._check_domain_command(Path("/repo"), domain, self.command(domain)), "cargo")
                wrapped = ["rtk", "proxy", *self.command(domain, "/usr/bin/cargo")]
                self.assertEqual(guard._check_domain_command(Path("/repo"), domain, wrapped), "/usr/bin/cargo")

    def test_check_recipes_reject_wrong_arms_overrides_and_other_cargo_verbs(self) -> None:
        valid = self.command("performance-baseline")
        invalid = [
            [*valid[:valid.index("baseline")], "candidate", *valid[valid.index("baseline") + 1:]],
            [*valid, "--features", "baseline"],
            [*valid[:valid.index("scripts/html/performance-harness/Cargo.toml")], "elsewhere/Cargo.toml",
             *valid[valid.index("scripts/html/performance-harness/Cargo.toml") + 1:]],
            ["cargo", "check", *valid[2:]],
            ["cargo", "+nightly", *valid[1:]],
            ["cargo", "clippy", "--release", *valid[3:]],
            ["cargo", "clippy", "--target", "aarch64-apple-darwin", *valid[3:]],
            ["cargo", "clippy", "--config", "build.target='x86_64-unknown-linux-gnu'", *valid[3:]],
            ["cargo", "clippy", "-j", "0", *valid[4:]],
            valid[:-2],
        ]
        for command in invalid:
            with self.subTest(command=command), self.assertRaises(RuntimeError):
                guard._check_domain_command(Path("/repo"), "performance-baseline", command)

    def test_check_domain_retains_only_canonical_root_coverage_profile(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target = root / "target"
            profile = target / "llvm-cov-target/krr-coverage"
            profile.mkdir(parents=True)

            self.assertEqual(
                guard.check_domain_retained_profiles(root, "root", [str(profile)]),
                (profile,),
            )
            self.assertEqual(guard.check_domain_retained_profiles(root, "root", []), ())

            rejected = (
                ("unsupported-domain", "performance-baseline", [str(profile)]),
                ("arbitrary-profile", "root", [str(target / "debug")]),
                ("duplicate-profile", "root", [str(profile), str(profile)]),
            )
            for label, domain, values in rejected:
                with self.subTest(case=label), self.assertRaisesRegex(RuntimeError, "canonical root coverage"):
                    guard.check_domain_retained_profiles(root, domain, values)

            shutil.rmtree(profile)
            external = root / "external-profile"
            external.mkdir()
            profile.symlink_to(external, target_is_directory=True)
            with self.assertRaisesRegex(RuntimeError, "directory kind or device boundary"):
                guard.check_domain_retained_profiles(root, "root", [str(profile)])

    @staticmethod
    def create_real_workspace(root: Path) -> dict[str, str]:
        runtime = root / "crates/katana-render-runtime"
        (runtime / "src").mkdir(parents=True)
        (runtime / "src/lib.rs").write_text("pub fn value() -> usize { 7 }\n")
        (root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["crates/katana-render-runtime"]\nresolver = "2"\n'
        )
        (runtime / "Cargo.toml").write_text(
            '[package]\nname = "katana-render-runtime"\nversion = "0.1.0"\nedition = "2021"\n'
        )
        harness = root / "scripts/html/performance-harness"
        (harness / "src").mkdir(parents=True)
        (harness / "src/main.rs").write_text("fn main() { println!(\"fixture\"); }\n")
        (harness / "Cargo.toml").write_text(
            '[package]\nname = "harness-fixture"\nversion = "0.1.0"\nedition = "2021"\n'
            '\n[workspace]\n\n[features]\nbaseline = []\ncandidate = []\n'
        )
        cargo = shutil.which("cargo")
        if cargo is None:
            raise AssertionError("real Cargo is required")
        env = dict(os.environ)
        env.pop("CARGO_CAPACITY_PROTECTED_REFS", None)
        env["CARGO_HOME"] = str(root / ".cargo-home")
        env["CARGO_TARGET_DIR"] = str(root / "target")
        env["RUSTFLAGS"] = "-D warnings"
        for manifest in (root / "Cargo.toml", harness / "Cargo.toml"):
            lock = subprocess.run([cargo, "generate-lockfile", "--offline", "--manifest-path", str(manifest)],
                                  cwd=root, env=env, text=True, capture_output=True, timeout=60, check=False)
            if lock.returncode != 0:
                raise AssertionError(lock.stderr)
        return env

    @contextmanager
    def isolated_workspace(self, *, capacity_factor: bool = False) -> Iterator[tuple[Path, dict[str, str]]]:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            environment = self.create_real_workspace(root)
            if capacity_factor:
                environment["CARGO_CAPACITY_FACTOR"] = str(guard.DEFAULT_FACTOR)
            with patch.dict(os.environ, environment, clear=True):
                yield root, environment

    def require_successful_initial_root_clippy(self, root: Path, environment: dict[str, str]) -> None:
        first = self.run_guard_clippy(root, environment, "root")
        self.assertEqual(first.returncode, 0, first.stderr)

    @unittest.skipIf(os.name == "nt", "isolated Cargo fixture uses POSIX target accounting")
    def test_real_isolated_metadata_and_clippy_for_all_domains(self) -> None:
        cargo = shutil.which("cargo")
        if cargo is None:
            self.fail("real Cargo is required")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            environment = self.create_real_workspace(root)
            for domain in guard.CHECK_DOMAINS:
                with self.subTest(domain=domain), patch.dict(os.environ, environment, clear=True):
                    metadata, verify = guard.check_domain_selection(
                        root, root / "target", domain, self.command(domain), environment,
                    )
                    self.assertEqual(metadata["target_directory"], str(root / "target"))
                    child = subprocess.run([cargo, *self.command(domain)[1:]], cwd=root, env=environment,
                                           text=True, capture_output=True, timeout=120, check=False)
                    self.assertEqual(child.returncode, 0, child.stderr)
                    verify()

    @staticmethod
    def capacity_state(root: Path, target: Path, baseline: int) -> bytes:
        state = root / "tmp/.cargo-capacity-guard.json"
        state.parent.mkdir(parents=True, exist_ok=True)
        content = json.dumps({
            "baseline_bytes": baseline,
            "factor": guard.DEFAULT_FACTOR,
            "target": str(target),
            "metric": guard.CAPACITY_METRIC,
            "metric_version": guard.CAPACITY_METRIC_VERSION,
        }, sort_keys=True).encode()
        state.write_bytes(content)
        return content

    @staticmethod
    def measured_bytes(path: Path) -> int:
        value = guard.size_bytes(path)
        if value is None:
            raise AssertionError(f"allocated bytes are unavailable for {path}")
        return value

    def run_guard_clippy(self, root: Path, environment: dict[str, str], domain: str,
                         protected_ref: Path | None = None,
                         dry_run: bool = False) -> subprocess.CompletedProcess[str]:
        command = [sys.executable, "-B", guard.__file__, "--check-domain", domain]
        if dry_run:
            command.append("--dry-run")
        if protected_ref is not None:
            command += ["--protected-ref", str(protected_ref)]
        command += ["--", *self.command(domain)]
        env = dict(environment, CARGO_CAPACITY_GUARD_ROOT=str(root))
        return subprocess.run(command, cwd=root, env=env, text=True, capture_output=True,
                              timeout=120, check=False)

    @staticmethod
    def target_snapshot(target: Path) -> dict[str, tuple[int, int, int, int]]:
        result: dict[str, tuple[int, int, int, int]] = {}
        for path in (target, *target.rglob("*")):
            value = path.lstat()
            if path.is_dir():
                continue
            result[str(path.relative_to(target.parent))] = (
                value.st_dev, value.st_ino, value.st_size,
                int(hashlib.sha256(path.read_bytes()).hexdigest()[:16], 16),
            )
        return result

    @unittest.skipIf(os.name == "nt", "isolated Cargo fixture uses POSIX target accounting")
    def test_insufficient_auxiliary_capacity_fails_before_target_mutation_or_child_clippy(self) -> None:
        with self.isolated_workspace(capacity_factor=True) as (root, environment):
            self.require_successful_initial_root_clippy(root, environment)
            target = root / "target"
            current = self.measured_bytes(target)
            baseline = max(1, (current - 1) // 2)
            state_before = self.capacity_state(root, target, baseline)
            target_before = self.target_snapshot(target)
            (root / "crates/katana-render-runtime/src/lib.rs").write_text(
                "pub fn value() -> usize { 8 }\n")
            second = self.run_guard_clippy(root, environment, "root")
            self.assertEqual(second.returncode, 1, second.stderr)
            self.assertIn("insufficient closed inactive artifact capacity", second.stderr)
            after = self.target_snapshot(target)
            self.assertTrue(set(target_before).issubset(after))
            self.assertEqual({name: after[name] for name in target_before}, target_before)
            self.assertEqual((root / "tmp/.cargo-capacity-guard.json").read_bytes(), state_before)

    @unittest.skipIf(os.name == "nt", "isolated Cargo fixture uses POSIX allocated-byte accounting")
    def test_explicit_cli_and_environment_references_remain_protected(self) -> None:
        with self.isolated_workspace(capacity_factor=True) as (root, environment):
            self.require_successful_initial_root_clippy(root, environment)
            harness = root / "scripts/html/performance-harness"
            payload = harness / "src/payload.bin"
            payload.write_bytes(hashlib.shake_256(b"real-cargo-inactive-profile").digest(1_120_000))
            (harness / "src/lib.rs").write_text(
                '#[used]\npub static PAYLOAD: [u8; 1_120_000] = '
                '*include_bytes!("payload.bin");\n'
            )
            cargo = shutil.which("cargo")
            if cargo is None:
                self.fail("real Cargo is required")
            release_build = subprocess.run(
                [cargo, "build", "--offline", "--release", "--manifest-path",
                 str(harness / "Cargo.toml"), "--message-format=json"],
                cwd=root, env=environment, text=True, capture_output=True, timeout=120,
                check=False,
            )
            self.assertEqual(release_build.returncode, 0,
                             f"{release_build.stderr}\n{release_build.stdout}")
            release_outputs = sorted((root / "target/release/deps").glob("libharness_fixture-*.rlib"))
            self.assertEqual(len(release_outputs), 1, "real Cargo must produce the inactive fixture rlib")
            self.assertGreater(release_outputs[0].stat().st_blocks * 512, 900_000)
            debug_outputs = sorted(
                (root / "target/debug/deps").glob("libkatana_render_runtime-*.r*")
            )
            self.assertGreaterEqual(
                len(debug_outputs), 2, "real Clippy must produce protected library outputs"
            )
            environment_reference, cli_reference = debug_outputs[:2]
            identities = {
                path: (path.stat().st_dev, path.stat().st_ino, hashlib.sha256(path.read_bytes()).digest())
                for path in (environment_reference, cli_reference)
            }
            current = self.measured_bytes(root / "target")
            debug_bytes = self.measured_bytes(root / "target/debug")
            release_bytes = self.measured_bytes(root / "target/release/deps")
            baseline = current - release_bytes + 1
            self.assertGreaterEqual(baseline, debug_bytes)
            self.assertGreater(current, baseline * guard.DEFAULT_FACTOR)
            self.capacity_state(root, root / "target", baseline)
            environment["CARGO_CAPACITY_PROTECTED_REFS"] = json.dumps([str(environment_reference)])
            result = self.run_guard_clippy(root, environment, "root", cli_reference, dry_run=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            report = json.loads(result.stderr.splitlines()[0])
            cleanup = report["finite_cleanup"]
            self.assertGreater(cleanup["retained_protected_inodes"], 0)
            selected = {root / "target" / path for path in cleanup["candidate_paths"]}
            self.assertIn(release_outputs[0], selected,
                          "cleanup must select the real inactive Cargo rlib")
            self.assertTrue({environment_reference, cli_reference}.isdisjoint(selected))
            for path, identity in identities.items():
                self.assertEqual((path.stat().st_dev, path.stat().st_ino,
                                  hashlib.sha256(path.read_bytes()).digest()), identity)

    @unittest.skipIf(os.name == "nt", "isolated Cargo fixture uses POSIX allocated-byte accounting")
    def test_insufficient_closed_auxiliary_capacity_reports_low_water_shortfall_before_real_clippy(self) -> None:
        with self.isolated_workspace(capacity_factor=True) as (root, environment):
            cargo = shutil.which("cargo")
            if cargo is None:
                self.fail("real Cargo is required")
            for value in (7, 8, 9):
                (root / "crates/katana-render-runtime/src/lib.rs").write_text(
                    f"pub fn value() -> usize {{ {value} }}\n")
                built = subprocess.run([cargo, *self.command("root")[1:]], cwd=root, env=environment,
                                       text=True, capture_output=True, timeout=120, check=False)
                self.assertEqual(built.returncode, 0, built.stderr)
            current = self.measured_bytes(root / "target")
            baseline = max(1, (current - 1) // 2)
            self.capacity_state(root, root / "target", baseline)
            result = self.run_guard_clippy(root, environment, "root")
            self.assertEqual(result.returncode, 1, result.stderr)
            report = json.loads(result.stderr.splitlines()[0])
            self.assertEqual(report["cleanup_status"], "failed")
            self.assertFalse(report["cleanup_goal_reached"])
            self.assertTrue(report["remaining_over_capacity"])
            self.assertGreater(report["finite_cleanup"]["estimated_candidate_bytes"], 0)

    def test_metadata_epoch_rejects_lock_config_and_environment_drift(self) -> None:
        for drift in ("lock", "config", "environment", "toolchain"):
            with self.subTest(drift=drift), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                environment = self.create_real_workspace(root)
                with patch.dict(os.environ, environment, clear=True):
                    _metadata, verify = guard.check_domain_selection(
                        root, root / "target", "performance-baseline",
                        self.command("performance-baseline"), environment,
                    )
                    if drift == "lock":
                        with (root / "scripts/html/performance-harness/Cargo.lock").open("a") as lock:
                            lock.write("\n")
                    elif drift == "config":
                        (root / ".cargo-home/config.toml").parent.mkdir(exist_ok=True)
                        (root / ".cargo-home/config.toml").write_text("[build]\nrustflags = '-C opt-level=3'\n")
                    elif drift == "toolchain":
                        (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "stable"\n')
                    else:
                        os.environ["CARGO_TARGET_DIR"] = str(root / "other-target")
                    with self.assertRaises(RuntimeError):
                        verify()

    def test_metadata_and_tool_queries_are_constant_across_cleanup_leaf_checks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            environment = self.create_real_workspace(root)
            with patch.dict(os.environ, environment, clear=True):
                original_run = subprocess.run
                with patch("subprocess.run", wraps=original_run) as counted_run:
                    _metadata, verify = guard.check_domain_selection(
                        root, root / "target", "performance-baseline",
                        self.command("performance-baseline"), environment,
                    )
                    admission_queries = counted_run.call_count
                    for _ in range(9):
                        verify()
                self.assertEqual(admission_queries, 9)
                self.assertEqual(counted_run.call_count, admission_queries)

    def test_root_metadata_scope_reuses_one_fresh_selected_input_snapshot(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            environment = self.create_real_workspace(root)
            source = root / "crates/katana-render-runtime/src/lib.rs"
            snapshot_checks = 0
            original = guard._selected_input_check

            def tracked_check(
                selected_root: Path,
                metadata: guard.JsonObject,
                *,
                include_target_inputs: bool = False,
            ) -> Callable[[], None]:
                check = original(selected_root, metadata, include_target_inputs=include_target_inputs)

                def verify() -> None:
                    nonlocal snapshot_checks
                    snapshot_checks += 1
                    check()

                return verify

            with patch.dict(os.environ, environment, clear=True):
                with patch("subprocess.run", wraps=subprocess.run) as subprocess_calls:
                    with patch(
                        "cargo_capacity_guard._selected_input_check",
                        side_effect=tracked_check,
                    ) as checks:
                        metadata, verify = guard.check_domain_selection(
                            root, root / "target", "root", self.command("root"), environment,
                        )
                        metadata_calls = [
                            call for call in subprocess_calls.call_args_list
                            if len(call.args[0]) > 1 and call.args[0][1] == "metadata"
                        ]
                        self.assertEqual(len(metadata_calls), 2)
                        self.assertEqual(metadata["workspace_root"], str(root))
                        self.assertEqual(checks.call_count, 1)
                        verify()
                        self.assertEqual(snapshot_checks, 1)
                        source.write_text(source.read_text() + "// changed after admission\n")
                        with self.assertRaisesRegex(RuntimeError, "selected metadata inputs changed"):
                            verify()
                        self.assertEqual(snapshot_checks, 2)

    def test_metadata_epoch_rejects_auto_target_and_tool_binary_identity_drift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            environment = self.create_real_workspace(root)
            with patch.dict(os.environ, environment, clear=True):
                _metadata, verify = guard.check_domain_selection(
                    root, root / "target", "root", self.command("root"), environment,
                )
                added_target = root / "crates/katana-render-runtime/src/bin/new-target.rs"
                added_target.parent.mkdir(parents=True)
                added_target.write_text("fn main() {}\n")
                with self.assertRaises(RuntimeError):
                    verify()

    def test_metadata_epoch_rejects_absent_build_script_added_after_admission(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            environment = self.create_real_workspace(root)
            build_script = root / "crates/katana-render-runtime/build.rs"
            self.assertFalse(build_script.exists())
            with patch.dict(os.environ, environment, clear=True):
                _metadata, verify = guard.check_domain_selection(
                    root, root / "target", "root", self.command("root"), environment,
                )
                build_script.write_text("fn main() {}\n")
                with self.assertRaises(RuntimeError):
                    verify()

    def test_metadata_epoch_pins_explicit_target_source_outside_conventional_roots(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            environment = self.create_real_workspace(root)
            manifest = root / "crates/katana-render-runtime/Cargo.toml"
            manifest.write_text(
                manifest.read_text() + '\n[[bin]]\nname = "custom_probe"\npath = "custom.rs"\n'
            )
            custom_source = manifest.parent / "custom.rs"
            custom_source.write_text("fn main() {}\n")
            with patch.dict(os.environ, environment, clear=True):
                _metadata, verify = guard.check_domain_selection(
                    root, root / "target", "root", self.command("root"), environment,
                )
                custom_source.write_text("fn main() { println!(\"changed\"); }\n")
                with self.assertRaises(RuntimeError):
                    verify()

    def test_noncanonical_workspace_and_workspace_glob_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory).resolve()
            root = parent / "workspace"
            root.mkdir()
            environment = self.create_real_workspace(root)
            globbed = root / "Cargo.toml"
            globbed.write_text('[workspace]\nmembers = ["crates/*"]\nresolver = "2"\n')
            with patch.dict(os.environ, environment, clear=True):
                with self.assertRaisesRegex(RuntimeError, "workspace globs"):
                    guard.check_domain_selection(
                        root, root / "target", "root", self.command("root"), environment,
                    )

    def test_workspace_symlink_alias_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory).resolve()
            root = parent / "workspace"
            root.mkdir()
            environment = self.create_real_workspace(root)
            alias = parent / "workspace-alias"
            alias.symlink_to(root, target_is_directory=True)
            aliased_environment = dict(environment, CARGO_TARGET_DIR=str(alias / "target"))
            with patch.dict(os.environ, aliased_environment, clear=True):
                with self.assertRaisesRegex(RuntimeError, "symlink|another workspace"):
                    guard.check_domain_selection(
                        alias, alias / "target", "root", self.command("root"), aliased_environment,
                    )

    def test_same_version_physical_tool_replacement_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            original = Path(sys.executable).resolve()
            replacement = root / "tool"
            shutil.copy2(original, replacement)
            pin = source_pin(replacement)
            data = replacement.read_bytes()
            replacement.write_bytes(data[:-1] + bytes([data[-1] ^ 1]))
            self.assertNotEqual(source_pin(replacement), pin)
            with self.assertRaises(RuntimeError):
                guard._verify_check_domain_tool_pins((
                    (replacement, pin, "cargo 1.0"),
                    (replacement, pin, "rustc 1.0"),
                    (pin, "cargo-clippy 1.0"),
                    (pin, "clippy 1.0"), pin,
                ))

    def test_check_domain_rejects_empty_reference_environment_and_unsupported_overrides(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            environment = self.create_real_workspace(root)
            environment["CARGO_CAPACITY_PROTECTED_REFS"] = "[]"
            with patch.dict(os.environ, environment, clear=True):
                duplicate = subprocess.run(
                    [sys.executable, "-B", guard.__file__, "--check-domain", "root",
                     "--check-domain", "root"],
                    cwd=root, env=dict(environment, CARGO_CAPACITY_GUARD_ROOT=str(root)),
                    text=True, capture_output=True, timeout=30, check=False,
                )
                self.assertEqual(duplicate.returncode, 2, duplicate.stderr)
                self.assertIn("must be declared once", duplicate.stderr)
                empty_refs = self.run_guard_clippy(root, environment, "root")
                self.assertEqual(empty_refs.returncode, 2, empty_refs.stderr)
                self.assertIn("CARGO_CAPACITY_PROTECTED_REFS", empty_refs.stderr)
                environment.pop("CARGO_CAPACITY_PROTECTED_REFS")
                os.environ.pop("CARGO_CAPACITY_PROTECTED_REFS")
                for name, value in (("RUSTUP_TOOLCHAIN", "nightly"),
                                    ("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS", "-C opt-level=3"),
                                    ("RUSTFLAGS", "")):
                    with self.subTest(name=name):
                        invalid = dict(environment, **{name: value})
                        with self.assertRaises(RuntimeError):
                            guard.check_domain_selection(
                                root, root / "target", "root", self.command("root"), invalid,
                            )
                (root / ".cargo-home").mkdir(exist_ok=True)
                (root / ".cargo-home/config.toml").write_text(
                    '[build]\nrustflags = "-C opt-level=3"\n')
                with self.assertRaisesRegex(RuntimeError, "compiler or Cargo configuration"):
                    guard.check_domain_selection(
                        root, root / "target", "root", self.command("root"), environment,
                    )


if __name__ == "__main__":
    unittest.main()
