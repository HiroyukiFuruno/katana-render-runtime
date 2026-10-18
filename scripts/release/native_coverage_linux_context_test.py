from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

import native_coverage_linux_context as linux_context


class LinuxCoverageContextTests(unittest.TestCase):
    def setUp(self) -> None:
        self.environment = {
            "CARGO_INCREMENTAL": "0",
            "CARGO_TERM_COLOR": "always",
            "RUSTFLAGS": "-D warnings",
            "TEST_THREADS": "1",
            "RUNNER_OS": "Linux",
            "RUNNER_ARCH": "X64",
            "ImageOS": "ubuntu-test",
            "ImageVersion": "2026.10.17.1",
            "GITHUB_WORKFLOW_SHA": "a" * 40,
        }

    def test_rejects_non_linux_and_unreviewed_architecture(self) -> None:
        for system, machine in (("Darwin", "arm64"), ("Linux", "aarch64")):
            with self.subTest(system=system, machine=machine), self.assertRaisesRegex(
                linux_context.LinuxCoverageContextError, "only the Linux x86_64"
            ):
                linux_context._validate_environment(self.environment, system, machine)

    def test_fixed_workflow_environment_and_unknown_overrides_fail_closed(self) -> None:
        self.environment["RUSTFLAGS"] = "-C opt-level=3"
        with self.assertRaisesRegex(linux_context.LinuxCoverageContextError, "fixed test environment"):
            linux_context._validate_environment(self.environment, "Linux", "x86_64")
        self.environment["RUSTFLAGS"] = "-D warnings"
        self.environment["RUSTC_WRAPPER"] = "/private/wrapper"
        with self.assertRaisesRegex(linux_context.LinuxCoverageContextError, "unsupported toolchain"):
            linux_context._validate_environment(self.environment, "Linux", "x86_64")

    def test_external_tool_selectors_and_runtime_overrides_fail_closed(self) -> None:
        for name in ("LLVM_COV", "LLVM_PROFDATA", "LLVM_PROFILE_FILE", "_JAVA_OPTIONS", "LD_AUDIT"):
            with self.subTest(name=name):
                self.environment[name] = "unreviewed-override"
                with self.assertRaisesRegex(linux_context.LinuxCoverageContextError, "unsupported toolchain"):
                    linux_context._validate_environment(self.environment, "Linux", "x86_64")
                self.environment[name] = ""
                self.assertEqual(
                    linux_context._validate_environment(self.environment, "Linux", "x86_64")["runnerOs"],
                    "Linux",
                )
                del self.environment[name]

    def test_unlisted_cargo_and_rust_prefix_overrides_fail_closed(self) -> None:
        for name in ("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS", "RUST_TEST_THREADS"):
            for value in ("override", ""):
                with self.subTest(name=name, value=value):
                    self.environment[name] = value
                    with self.assertRaisesRegex(linux_context.LinuxCoverageContextError, "Cargo or Rust override"):
                        linux_context._validate_environment(self.environment, "Linux", "x86_64")
                    del self.environment[name]

    def test_context_profile_is_safe_and_nonce_independent(self) -> None:
        profile = linux_context._validate_environment(self.environment, "Linux", "x86_64")
        self.assertEqual(profile, {
            "runnerOs": "Linux", "runnerArch": "X64", "imageOS": "ubuntu-test",
            "imageVersion": "2026.10.17.1", "testThreads": "1",
        })
        self.environment["GITHUB_RUN_ID"] = "1234567"
        self.assertEqual(profile, linux_context._validate_environment(self.environment, "Linux", "x86_64"))
        first = linux_context._environment_hash({"locale": "C.UTF-8", "tool": "a" * 64})
        second = linux_context._environment_hash({"tool": "a" * 64, "locale": "C.UTF-8"})
        changed = linux_context._environment_hash({"locale": "C.UTF-8", "tool": "b" * 64})
        self.assertEqual(first, second)
        self.assertNotEqual(first, changed)

    def test_safe_version_summary_rejects_paths_and_multiline_data(self) -> None:
        self.assertEqual(linux_context._safe_text("rustc 1.90.0\n", "rustc"), "rustc 1.90.0")
        with self.assertRaisesRegex(linux_context.LinuxCoverageContextError, "safe summary"):
            linux_context._safe_text("/private/tool 1.2", "tool")

    def test_opaque_debian_version_hash_accepts_epoch_and_tilde_without_exposing_value(self) -> None:
        value = "2:2.40.1-1~deb12u1"
        digest = linux_context._opaque_version_digest(value, "package")
        self.assertRegex(digest, r"^[0-9a-f]{64}$")
        self.assertNotIn(value, digest)
        with self.assertRaisesRegex(linux_context.LinuxCoverageContextError, "version is invalid"):
            linux_context._opaque_version_digest("\n", "package")

    def test_fontconfig_closure_parser_requires_explicit_absolute_paths(self) -> None:
        parsed = linux_context._fontconfig_config_paths("+ /etc/fonts/fonts.conf: enabled\n- /etc/fonts/old.conf: disabled")
        self.assertEqual(parsed, [Path("/etc/fonts/fonts.conf"), Path("/etc/fonts/old.conf")])
        for malformed in ("", "+ relative.conf: enabled", "unexpected output"):
            with self.subTest(malformed=malformed), self.assertRaisesRegex(
                linux_context.LinuxCoverageContextError, "fontconfig configuration closure"
            ):
                linux_context._fontconfig_config_paths(malformed)

    def test_selected_rustup_llvm_version_uses_pinned_sysroot_path(self) -> None:
        with TemporaryDirectory() as temporary:
            llvm_cov = Path(temporary) / "llvm-cov"
            llvm_cov.write_text("#!/bin/sh\nprintf 'LLVM version 18.1.8\\n'\n")
            llvm_cov.chmod(0o755)
            records: dict[str, str] = {}
            self.assertEqual(linux_context._record_llvm_version(records, llvm_cov), "llvm 18.1.8")
            self.assertIn("rustup-llvm:llvm-cov-selected", records)
            self.assertNotIn(str(llvm_cov), str(records))

    def test_record_tree_follows_directory_symlinks_and_rejects_cycles(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / "rules"
            target.mkdir()
            (target / "fonts.conf").write_text("<fontconfig/>\n")
            alias = root / "conf.d"
            alias.symlink_to(target, target_is_directory=True)
            records: dict[str, str] = {}
            linux_context._record_tree(alias, records, "fontconfig/conf.d")
            self.assertIn("fontconfig/conf.d/fonts.conf", records)
            (target / "cycle").symlink_to(target, target_is_directory=True)
            with self.assertRaisesRegex(linux_context.LinuxCoverageContextError, "directory cycle"):
                linux_context._record_tree(target, {}, "fontconfig/cycle")

    def test_font_inventory_binds_file_bytes_and_rejects_missing_entries(self) -> None:
        with TemporaryDirectory() as temporary:
            font = Path(temporary) / "fixture.font"
            font.write_bytes(b"font bytes v1")
            first_records: dict[str, str] = {}
            row = f"Fixture Sans\tRegular\t0\t{font}"
            first = linux_context._font_inventory_digest(row, first_records)
            font.write_bytes(b"font bytes v2")
            second = linux_context._font_inventory_digest(row, {})
            self.assertNotEqual(first, second)
            with self.assertRaisesRegex(linux_context.LinuxCoverageContextError, "unavailable font"):
                linux_context._font_inventory_digest("Fixture Sans\tRegular\t0\tmissing.font", {})


if __name__ == "__main__":
    unittest.main()
