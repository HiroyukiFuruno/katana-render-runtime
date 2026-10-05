from __future__ import annotations

import unittest
import os
import shlex
import subprocess
import tempfile
import textwrap
from pathlib import Path


class ReleaseWorkflowContractTest(unittest.TestCase):
    def setUp(self) -> None:
        root = Path(__file__).parents[2]
        self.release = (root / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.preflight = (root / ".github/workflows/release-preflight.yml").read_text(encoding="utf-8")
        self.retry = (root / ".github/workflows/release-publish-retry.yml").read_text(encoding="utf-8")
        self.publisher = (root / "scripts/release/publish-crates.sh").read_text(encoding="utf-8")
        self.justfile = (root / "Justfile").read_text(encoding="utf-8")
        self.ci = (root / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")

    def test_ubuntu_coverage_owns_unit_tests_without_manual_cache_purges(self) -> None:
        unit_step = self.workflow_step(self.ci, "Run tests")
        self.assertIn("if: matrix.os != 'ubuntu-latest'", unit_step)
        self.assertIn("run: just unit-test", unit_step)
        coverage_step = self.workflow_step(self.ci, "Run coverage")
        self.assertIn("if: matrix.os == 'ubuntu-latest'", coverage_step)
        self.assertIn("run: just coverage", coverage_step)
        self.assertIn("timeout-minutes: 45", coverage_step)

        self.assertIn("uses: Swatinem/rust-cache@v2.9.2", self.ci)
        cache = self.workflow_step(self.ci, "Cache Rust build outputs")
        self.assertNotIn("if: matrix.os != 'ubuntu-latest'", cache)
        self.assertIn("shared-key: ci-v2-${{ matrix.os }}-stable-default", cache)
        for platform in ("linux64", "mac-arm64", "win64"):
            self.assertIn(f"platform: {platform}", self.ci)
        self.assertIn("- os: macos-15\n            platform: mac-arm64", self.ci)
        self.assertNotIn("platform: mac-x64", self.ci)
        self.assertNotIn("os: macos-15-intel", self.ci)
        self.assertNotRegex(self.ci, r"rm\s+-rf\s+(?:\S*/)?target(?:/|\s|$)")

        for name in (
            "Free Ubuntu build space before coverage",
            "Free Ubuntu build space after coverage",
        ):
            self.assertNotIn(f"- name: {name}", self.ci)
        for step_name in (
            "Record Ubuntu build disk usage before coverage",
            "Record Ubuntu build disk usage after coverage",
        ):
            step = self.workflow_step(self.ci, step_name)
            self.assertIn("df -h .", step)
            self.assertIn("du -sh target", step)
        self.assertIn(
            "if: matrix.os == 'ubuntu-latest' && always()",
            self.workflow_step(self.ci, "Record Ubuntu build disk usage after coverage"),
        )
        record = self.workflow_step(self.ci, "Record immutable PR base evidence")
        upload = self.workflow_step(self.ci, "Upload immutable PR base evidence")
        for evidence_step in (record, upload):
            self.assertIn("github.event_name == 'pull_request'", evidence_step)
            self.assertIn("matrix.os == 'ubuntu-latest'", evidence_step)
            self.assertIn("success()", evidence_step)
        self.assertLess(
            self.ci.index("Record immutable PR base evidence"),
            self.ci.index("Upload immutable PR base evidence"),
        )

    def test_ubuntu_repairs_only_a_missing_cached_v8_archive_before_first_cargo_link(self) -> None:
        repair = self.workflow_step(self.ci, "Repair cached V8 archive on Ubuntu")
        self.assertIn("if: matrix.os == 'ubuntu-latest'", repair)
        script = self.workflow_script(self.ci, "Repair cached V8 archive on Ubuntu")
        self.assertIn("for target_dir in target target/llvm-cov-target; do", script)
        self.assertIn(
            'if [ ! -s "$target_dir/debug/gn_out/obj/librusty_v8.a" ]', script
        )
        self.assertIn('cargo clean --target-dir "$target_dir" -p v8', script)
        self.assertNotIn("rm -rf", repair)
        self.assertLess(
            self.ci.index("Repair cached V8 archive on Ubuntu"),
            self.ci.index("Check Rust types"),
        )

    def test_ubuntu_v8_repair_cleans_only_targets_with_missing_or_empty_archives(self) -> None:
        script = self.workflow_script(self.ci, "Repair cached V8 archive on Ubuntu")
        cases = (
            (None, None, []),
            ("target", None, ["clean --target-dir target -p v8"]),
            (
                "target/llvm-cov-target",
                None,
                ["clean --target-dir target/llvm-cov-target -p v8"],
            ),
            (
                None,
                "target/llvm-cov-target",
                ["clean --target-dir target/llvm-cov-target -p v8"],
            ),
            (None, "target", ["clean --target-dir target -p v8"]),
        )
        for missing_target, empty_target, expected_calls in cases:
            with self.subTest(missing_target=missing_target, empty_target=empty_target):
                with tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    for target_dir in ("target", "target/llvm-cov-target"):
                        archive = (
                            root
                            / target_dir
                            / "debug/gn_out/obj/librusty_v8.a"
                        )
                        if target_dir != missing_target:
                            archive.parent.mkdir(parents=True, exist_ok=True)
                            archive.write_bytes(
                                b"" if target_dir == empty_target else b"archive"
                            )

                    binary_dir = root / "bin"
                    binary_dir.mkdir()
                    cargo = binary_dir / "cargo"
                    cargo.write_text(
                        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CARGO_CALL_LOG\"\n",
                        encoding="utf-8",
                    )
                    cargo.chmod(0o755)
                    call_log = root / "cargo-calls.log"
                    environment = os.environ.copy()
                    environment["PATH"] = f"{binary_dir}:{environment['PATH']}"
                    environment["CARGO_CALL_LOG"] = str(call_log)

                    subprocess.run(
                        ["bash", "-euo", "pipefail", "-c", script],
                        cwd=root,
                        env=environment,
                        check=True,
                        capture_output=True,
                        text=True,
                    )

                    calls = (
                        call_log.read_text(encoding="utf-8").splitlines()
                        if call_log.exists()
                        else []
                    )
                    self.assertEqual(calls, expected_calls)

    def test_coverage_uses_official_clean_and_full_gate_without_wiping_build_cache(self) -> None:
        root = Path(__file__).parents[2]
        lines = (root / "Justfile").read_text(encoding="utf-8").splitlines()
        recipe_start = next(
            index for index, line in enumerate(lines) if line.startswith("coverage:")
        )
        recipe_commands = []
        for line in lines[recipe_start + 1 :]:
            if line.startswith("    "):
                recipe_commands.append(line.strip())
            elif not line:
                continue
            else:
                break

        parsed_commands = [shlex.split(command) for command in recipe_commands]
        coverage_commands = [tokens for tokens in parsed_commands if "llvm-cov" in tokens]
        cleanup_commands = [tokens for tokens in coverage_commands if "clean" in tokens]
        measurement_commands = [
            tokens for tokens in coverage_commands if "clean" not in tokens
        ]
        self.assertEqual(len(cleanup_commands), 1)
        self.assertEqual(len(measurement_commands), 1)
        cleanup = cleanup_commands[0]
        coverage = measurement_commands[0]
        self.assertEqual(cleanup[cleanup.index("llvm-cov") + 1], "clean")
        self.assertIn("--workspace", cleanup)
        self.assertLess(parsed_commands.index(cleanup), parsed_commands.index(coverage))

        required_options = (
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
            "--summary-only",
            "--fail-under-lines",
            "--fail-uncovered-lines",
        )
        for option in required_options:
            with self.subTest(option=option):
                self.assertIn(option, coverage)
        self.assertEqual(
            coverage[coverage.index("--fail-under-lines") + 1],
            "{{COVERAGE_MIN_LINES}}",
        )
        self.assertIn(
            "{{COVERAGE_MAX_UNCOVERED_LINES}}",
            coverage[coverage.index("--fail-uncovered-lines") + 1],
        )

        forbidden_options = ("--no-clean", "--no-report", "--no-run")
        for tokens in coverage_commands:
            for option in forbidden_options:
                self.assertNotIn(option, tokens)
        for tokens in parsed_commands:
            if tokens and tokens[0] == "rm":
                self.assertFalse(
                    any(
                        argument == "target"
                        or argument.startswith(("target/", "./target/"))
                        for argument in tokens[1:]
                    ),
                    "coverage must retain Cargo build caches",
                )
            if len(tokens) > 1 and tokens[1] == "clean":
                self.assertIn("llvm-cov", tokens)

    @staticmethod
    def workflow_step(workflow: str, name: str) -> str:
        marker = f"      - name: {name}\n"
        start = workflow.index(marker)
        end = workflow.find("\n      - ", start + len(marker))
        return workflow[start:] if end < 0 else workflow[start:end]

    @classmethod
    def workflow_script(cls, workflow: str, name: str) -> str:
        step = cls.workflow_step(workflow, name)
        lines = step.splitlines()
        run_index = lines.index("        run: |\n".rstrip())
        script_lines = []
        for line in lines[run_index + 1 :]:
            if line and not line.startswith("          "):
                break
            script_lines.append(line)
        return textwrap.dedent("\n".join(script_lines))

    def test_release_checks_install_the_bun_resolver_before_freshness_validation(self) -> None:
        for job in (self.release[self.release.index("  release-context:"):self.release.index("  release:\n")], self.release[self.release.index("  release:\n"):]):
            self.assertIn("uses: oven-sh/setup-bun@v2", job)
            self.assertIn("bun-version: 1.4.2", job)
            self.assertIn("run: bun install --frozen-lockfile", job)

    def test_release_target_check_requires_dependency_freshness_before_tagging(self) -> None:
        justfile = (Path(__file__).parents[2] / "Justfile").read_text(encoding="utf-8")
        freshness = justfile.index("python3 scripts/release/verify_dependency_freshness.py")
        target = justfile.index("python3 scripts/release/verify-release-target.py")
        self.assertLess(freshness, target)
        freshness_steps = (
            (self.release, "Release target check", 'run: just VERSION="${{ steps.version.outputs.version }}" release-target-check'),
            (self.release, "Verify release package", 'run: just VERSION="${{ needs.release-context.outputs.version }}" release-verify'),
            (self.preflight, "Release check", 'run: just VERSION="${{ steps.version.outputs.version }}" release-check'),
            (self.preflight, "Release preflight checks", 'run: just VERSION="${{ steps.version.outputs.version }}" release-preflight-check'),
        )
        for workflow, name, command in freshness_steps:
            step = self.workflow_step(workflow, name)
            self.assertIn(command, step, name)
            self.assertIn("env:\n          GH_TOKEN: ${{ github.token }}", step, name)
        self.assertNotIn("- name: Release quality gate", self.preflight)
        self.assertNotIn("- name: Release-specific verification", self.preflight)

    def test_release_preflight_uses_read_only_permissions_for_api_lookups(self) -> None:
        permissions = self.preflight[self.preflight.index("permissions:"):self.preflight.index("\nenv:")]
        self.assertEqual(
            permissions,
            "permissions:\n  actions: read\n  contents: read\n  pull-requests: read\n",
        )

    def test_public_release_and_cleanup_follow_both_registry_publications(self) -> None:
        publish = self.release.index("- name: Publish crates.io")
        public = self.release.index("- name: Publish complete GitHub Release")
        cleanup = self.release.index("- name: Cleanup published release state")
        self.assertLess(publish, public)
        self.assertLess(public, cleanup)
        conditional = "if: github.event_name == 'pull_request' || inputs.publish_crates == true"
        self.assertGreaterEqual(self.release.count(conditional), 3)
        self.assertIn('while IFS= read -r package; do', self.publisher)
        self.assertIn('publish_if_needed "${package}"\n  wait_for_crate "${package}"', self.publisher)
        cleanup = self.workflow_step(self.release, "Cleanup published release state")
        self.assertNotIn("--delete-remote", cleanup)

    def test_retry_rejoins_the_same_publication_completion_path(self) -> None:
        publish = self.retry.index("- name: Publish crates.io from release tag")
        public = self.retry.index("- name: Publish complete GitHub Release")
        cleanup = self.retry.index("- name: Cleanup published release state")
        self.assertLess(publish, public)
        self.assertLess(public, cleanup)
        self.assertIn("contents: write", self.retry)
        retry_publish = self.retry[public:cleanup]
        self.assertIn('gh release edit "${TAG}" --repo "${GITHUB_REPOSITORY}" --draft=false --verify-tag', retry_publish)
        self.assertNotIn("--latest", retry_publish)
        self.assertIn("python3 scripts/release/cleanup_release_state.py", self.retry)
        cleanup_step = self.workflow_step(self.retry, "Cleanup published release state")
        self.assertNotIn("--delete-remote", cleanup_step)

    def test_release_package_plan_preserves_dependency_order_and_checks(self) -> None:
        packages = (
            "katana-render-runtime-assets",
            "katana-render-runtime",
            "katana-render-runtime-cli",
        )
        self.assertIn('required = ["katana-render-runtime", "katana-render-runtime-cli"]', self.publisher)
        self.assertIn('if parts >= (0, 4, 23):', self.publisher)
        self.assertIn('required = ["katana-render-runtime-assets", *required]', self.publisher)
        self.assertIn('for name in required:', self.publisher)
        self.assertIn('publish_if_needed "${package}"\n  wait_for_crate "${package}"', self.publisher)

        unpublished = (
            Path(__file__).parent / "assert-crates-not-published.sh"
        ).read_text(encoding="utf-8")
        self.assertIn("packages=(" + " ".join(packages) + ")", unpublished)
        internal = (
            Path(__file__).parent / "verify-internal-dependencies.sh"
        ).read_text(encoding="utf-8")
        for dependency in packages[:2]:
            self.assertIn(dependency, internal)

        release_verify = self.justfile[
            self.justfile.index("release-verify:"):self.justfile.index(
                "# Verify completed OpenSpec changes"
            )
        ]
        self.assertIn(
            "package -p " + " -p ".join(packages) + " --locked --allow-dirty",
            release_verify,
        )
        self.assertIn("python3 scripts/release/test_packaged_runtime.py", release_verify)
        self.assertIn("--version \"{{VERSION_BARE}}\"", release_verify)
        for package in packages:
            self.assertIn(f"verify-crate-size.sh {package}", release_verify)
        self.assertNotIn("publish -p", release_verify)

    def test_post_merge_release_check_allows_only_a_published_package_prefix(self) -> None:
        target_check = self.workflow_step(self.release, "Release target check")
        self.assertIn('ALLOW_PUBLISHED_PREFIX: "true"', target_check)
        self.assertIn(
            'bash scripts/release/assert-crates-not-published.sh "{{VERSION}}"',
            self.justfile,
        )

        guard = Path(__file__).parent / "assert-crates-not-published.sh"
        packages = [
            "katana-render-runtime-assets",
            "katana-render-runtime",
            "katana-render-runtime-cli",
        ]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            scripts = root / "scripts/release"
            scripts.mkdir(parents=True)
            (scripts / guard.name).write_bytes(guard.read_bytes())
            (scripts / "verify-version.sh").write_text(
                '#!/usr/bin/env bash\necho "version_bare=${1#v}"\n',
                encoding="utf-8",
            )
            cargo_bin = root / "bin"
            cargo_bin.mkdir()
            cargo = cargo_bin / "cargo"
            cargo.write_text(
                "#!/usr/bin/env python3\n"
                "import os, sys\n"
                "package = sys.argv[2].split('@')[0]\n"
                "published = os.environ.get('PUBLISHED', '').split(',')\n"
                "failed = os.environ.get('REGISTRY_FAILURE', '').split(',')\n"
                "if package in failed:\n"
                " print('error: registry request failed', file=sys.stderr); sys.exit(101)\n"
                "if package in published:\n"
                " sys.exit(0)\n"
                "print('error: could not find package in registry', file=sys.stderr); sys.exit(101)\n",
                encoding="utf-8",
            )
            cargo.chmod(0o755)

            def run(published: list[str], *, allow_prefix: bool = True,
                    registry_failure: list[str] | None = None) -> subprocess.CompletedProcess[str]:
                environment = os.environ.copy()
                environment.update({
                    "PATH": f"{cargo_bin}:{environment['PATH']}",
                    "PUBLISHED": ",".join(published),
                    "REGISTRY_FAILURE": ",".join(registry_failure or []),
                })
                if allow_prefix:
                    environment["ALLOW_PUBLISHED_PREFIX"] = "true"
                else:
                    environment.pop("ALLOW_PUBLISHED_PREFIX", None)
                return subprocess.run(
                    ["bash", str(scripts / guard.name), "v0.4.23"],
                    cwd=root, env=environment, capture_output=True, text=True, check=False,
                )

            for prefix_length in range(4):
                with self.subTest(prefix_length=prefix_length):
                    result = run(packages[:prefix_length])
                    self.assertEqual(result.returncode, 0, result.stderr)
            for gap in (
                [packages[1]],
                [packages[2]],
                [packages[0], packages[2]],
                packages[1:],
            ):
                with self.subTest(gap=gap):
                    result = run(gap)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("already published", result.stderr)
            strict = run([packages[0]], allow_prefix=False)
            self.assertNotEqual(strict.returncode, 0)
            self.assertIn("already published", strict.stderr)
            for package in packages:
                with self.subTest(registry_failure=package):
                    registry_error = run([], registry_failure=[package])
                    self.assertNotEqual(registry_error.returncode, 0)
                    self.assertIn("Could not determine", registry_error.stderr)

    def test_internal_dependency_check_rejects_package_version_drift(self) -> None:
        script = Path(__file__).parent / "verify-internal-dependencies.sh"
        manifests = {
            "Cargo.toml": """[workspace]
members = ["crates/katana-render-runtime-assets", "crates/katana-render-runtime", "crates/katana-render-runtime-cli"]

[workspace.package]
version = "0.4.23"

[workspace.dependencies]
katana-render-runtime-assets = { path = "crates/katana-render-runtime-assets", version = "=0.4.23" }
katana-render-runtime = { path = "crates/katana-render-runtime", version = "0.4.23" }
""",
            "crates/katana-render-runtime-assets/Cargo.toml": """[package]
name = "katana-render-runtime-assets"
version.workspace = true
""",
            "crates/katana-render-runtime/Cargo.toml": """[package]
name = "katana-render-runtime"
version.workspace = true

[dependencies]
katana-render-runtime-assets = { workspace = true }
""",
            "crates/katana-render-runtime-cli/Cargo.toml": """[package]
name = "katana-render-runtime-cli"
version.workspace = true

[dependencies]
katana-render-runtime = { workspace = true }
""",
        }

        def run_check(
            package_version_line: str = "version.workspace = true",
            asset_dependency_requirement: str = "=0.4.23",
        ) -> subprocess.CompletedProcess[str]:
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                for relative, content in manifests.items():
                    path = root / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(content, encoding="utf-8")
                workspace_manifest = root / "Cargo.toml"
                workspace_manifest.write_text(
                    manifests["Cargo.toml"].replace(
                        'version = "=0.4.23"',
                        f'version = "{asset_dependency_requirement}"',
                    ),
                    encoding="utf-8",
                )
                cli_manifest = root / "crates/katana-render-runtime-cli/Cargo.toml"
                cli_manifest.write_text(
                    manifests["crates/katana-render-runtime-cli/Cargo.toml"].replace(
                        "version.workspace = true", package_version_line
                    ),
                    encoding="utf-8",
                )
                return subprocess.run(
                    ["bash", str(script), "0.4.23"],
                    cwd=root,
                    capture_output=True,
                    text=True,
                    check=False,
                )

        with self.subTest(case="matching workspace version"):
            passed = run_check("version.workspace = true")
            self.assertEqual(passed.returncode, 0, passed.stderr)
        with self.subTest(case="exact assets version requirement"):
            exact = run_check()
            self.assertEqual(exact.returncode, 0, exact.stderr)
        with self.subTest(case="bare caret assets version requirement"):
            bare_caret = run_check(asset_dependency_requirement="0.4.23")
            self.assertNotEqual(bare_caret.returncode, 0)
            self.assertIn("must use exact version =0.4.23", bare_caret.stderr)
        with self.subTest(case="stale exact assets version requirement"):
            stale_exact = run_check(asset_dependency_requirement="=0.4.24")
            self.assertNotEqual(stale_exact.returncode, 0)
            self.assertIn("must use exact version =0.4.23", stale_exact.stderr)
        with self.subTest(case="explicit stale package version"):
            mismatch = run_check(package_version_line='version = "0.4.24"')
            self.assertNotEqual(mismatch.returncode, 0)
            self.assertIn("inherit the workspace package version", mismatch.stderr)
        with self.subTest(case="missing package version"):
            missing = run_check(package_version_line="")
            self.assertNotEqual(missing.returncode, 0)
            self.assertIn("inherit the workspace package version", missing.stderr)

    def test_runtime_package_asset_check_rejects_drawio_source_and_compressed_copy(self) -> None:
        start = self.justfile.index("runtime-package-asset-check:")
        end = self.justfile.index("# Verify repository hook and release cleanup contracts", start)
        recipe = self.justfile[start:end]
        package_list_line = next(
            line for line in recipe.splitlines() if "runtime_package_files=" in line
        )
        self.assertIn("package -p katana-render-runtime --locked --allow-dirty --list", package_list_line)
        reject_line = next(line for line in recipe.splitlines() if "grep -Eq" in line)
        self.assertIn('printf \'%s\\n\' "$runtime_package_files"', reject_line)
        self.assertIn(r"drawio\.min\.js(\.br)?$", reject_line)
        self.assertIn('assets/drawio.min.js.br', recipe)

    def run_plantuml_install(self, failures: int, checksum: str) -> tuple[subprocess.CompletedProcess[str], Path, int]:
        root = Path(__file__).parents[2]
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        temp_path = Path(temp.name)
        fake_bin = temp_path / "bin"
        fake_bin.mkdir()
        attempts = temp_path / "attempts"
        curl = fake_bin / "curl"
        curl.write_text(
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            f"attempts={attempts!s}\n"
            "count=0\n"
            "if [ -f \"$attempts\" ]; then count=$(cat \"$attempts\"); fi\n"
            "count=$((count + 1))\n"
            "printf '%s\\n' \"$count\" > \"$attempts\"\n"
            f"if [ \"$count\" -le {failures} ]; then exit 22; fi\n"
            "while [ \"$#\" -gt 0 ]; do\n"
            "  if [ \"$1\" = \"--output\" ]; then printf 'fixture' > \"$2\"; exit 0; fi\n"
            "  shift\n"
            "done\n"
            "exit 64\n",
            encoding="utf-8",
        )
        sha256sum = fake_bin / "sha256sum"
        sha256sum.write_text(
            "#!/usr/bin/env bash\n"
            f"printf '%s  %s\\n' '{checksum}' \"$1\"\n",
            encoding="utf-8",
        )
        curl.chmod(0o755)
        sha256sum.chmod(0o755)
        output = temp_path / "cache" / "plantuml.jar"
        environment = os.environ | {
            "PATH": f"{fake_bin}{os.pathsep}{os.environ['PATH']}",
            "KRR_PLANTUML_DOWNLOAD_RETRY_DELAY_SECONDS": "0",
        }
        completed = subprocess.run(
            ["just", "plantuml-install", "1.2026.8", str(output)],
            cwd=root,
            env=environment,
            text=True,
            capture_output=True,
            check=False,
        )
        return completed, output, int(attempts.read_text(encoding="utf-8"))

    def test_plantuml_install_recovers_from_a_transient_http_failure(self) -> None:
        completed, output, attempts = self.run_plantuml_install(
            failures=1,
            checksum="1057dd8b346bed26a48ffebe6054e16fc785dda7c91f37f6c19030a4aab8a942",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(attempts, 2)
        self.assertEqual(output.read_text(encoding="utf-8"), "fixture")
        self.assertFalse(output.with_suffix(".jar.tmp").exists())

    def test_plantuml_install_fails_closed_after_bounded_retries(self) -> None:
        completed, output, attempts = self.run_plantuml_install(
            failures=3,
            checksum="1057dd8b346bed26a48ffebe6054e16fc785dda7c91f37f6c19030a4aab8a942",
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(attempts, 3)
        self.assertFalse(output.exists())
        self.assertFalse(output.with_suffix(".jar.tmp").exists())

    def test_plantuml_install_keeps_checksum_validation_after_recovery(self) -> None:
        completed, output, attempts = self.run_plantuml_install(failures=1, checksum="0" * 64)
        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(attempts, 2)
        self.assertFalse(output.exists())
        self.assertFalse(output.with_suffix(".jar.tmp").exists())


if __name__ == "__main__":
    unittest.main()
