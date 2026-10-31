from __future__ import annotations

import unittest
import hashlib
import os
import json
import re
import shlex
import subprocess
import tempfile
import textwrap
from pathlib import Path

PLANTUML_FIXTURE_SHA256 = hashlib.sha256(b"fixture").hexdigest()


class ReleaseWorkflowContractTest(unittest.TestCase):
    def setUp(self) -> None:
        root = Path(__file__).parents[2]
        self.release = (root / ".github/workflows/release.yml").read_text(
            encoding="utf-8"
        )
        self.preflight = (root / ".github/workflows/release-preflight.yml").read_text(
            encoding="utf-8"
        )
        self.retry = (root / ".github/workflows/release-publish-retry.yml").read_text(
            encoding="utf-8"
        )
        self.publisher = (root / "scripts/release/publish-crates.sh").read_text(
            encoding="utf-8"
        )
        self.justfile = (root / "Justfile").read_text(encoding="utf-8")
        self.ci = (root / ".github/workflows/test-and-build.yml").read_text(
            encoding="utf-8"
        )

    def test_owned_svg_model_tests_are_in_both_rustfmt_recipes(self) -> None:
        root = Path(__file__).parents[2]
        pattern = "crates/katana-render-runtime/src/svg_engine_*_model_tests.rs"
        targets = {path.resolve() for path in root.glob(pattern)}
        self.assertTrue(targets)
        vendor = root / "crates/katana-render-runtime/vendor/svg-engine"
        macro = re.compile(r'owned_svg_model_tests!\s*\(\s*"([^"\n]+)"\s*\)')
        referenced = {
            (path.parent / match).resolve()
            for path in vendor.rglob("*.rs")
            for match in macro.findall(path.read_text(encoding="utf-8"))
        }
        self.assertTrue(referenced)
        self.assertTrue(referenced <= targets, referenced - targets)
        child = re.compile(r'#\[path\s*=\s*"([^"\n]*_model_tests\.rs)"\]')
        children = {
            (path.parent / match).resolve()
            for path in targets
            for match in child.findall(path.read_text(encoding="utf-8"))
        }
        self.assertTrue(children <= targets, children - targets)
        for recipe, extra in (("fmt", []), ("fmt-check", ["--check"])):
            with self.subTest(recipe=recipe):
                commands = [shlex.split(line) for line in self.just_dry_run(recipe).splitlines()]
                self.assertIn(["rustfmt", "--edition", "2024", *extra, pattern], commands)

    def test_ubuntu_capacity_reference_precedes_cached_target_checks_and_coverage(
        self,
    ) -> None:
        name = "Initialize fixed capacity reference"
        step = self.workflow_step(self.ci, name)
        self.assertIn("if: matrix.os == 'ubuntu-latest'", step)
        self.assertEqual(
            shlex.split(step.split("run:", 1)[1].strip()),
            [
                "python3",
                "scripts/build/initialize_ci_capacity_state.py",
                "--root",
                "$PWD",
            ],
        )
        for later in ("Check Rust types", "Run coverage"):
            with self.subTest(later=later):
                self.assertLess(
                    self.ci.index(f"- name: {name}\n"),
                    self.ci.index(f"- name: {later}\n"),
                )

    def test_html_fidelity_recipes_preserve_bash_status_from_their_just_dry_run(
        self,
    ) -> None:
        root = Path(__file__).parents[2]
        for recipe in ("html-fidelity-check", "html-fidelity-interim-check"):
            with self.subTest(recipe=recipe):
                command = next(
                    line.strip()
                    for line in self.just_dry_run(recipe).splitlines()
                    if line.strip().startswith("status=0;")
                )
                terminal = command.rsplit(";", 1)[-1].strip()
                self.assertEqual(terminal, 'exit "$status"')
                for expected_status in (0, 1):
                    with self.subTest(status=expected_status):
                        completed = subprocess.run(
                            ["bash", "-c", f"status={expected_status}; {terminal}"],
                            cwd=root,
                            capture_output=True,
                            text=True,
                        )
                        self.assertEqual(completed.returncode, expected_status)

    def test_html_fidelity_recipes_skip_heavy_followups_after_failure(self) -> None:
        root = Path(__file__).parents[2]
        for recipe in ("html-fidelity-check", "html-fidelity-interim-check"):
            with self.subTest(recipe=recipe):
                command = next(
                    line.strip()
                    for line in self.just_dry_run(recipe).splitlines()
                    if line.strip().startswith("status=0;")
                )
                with tempfile.TemporaryDirectory() as temporary_directory:
                    calls = Path(temporary_directory) / "calls"
                    stub = textwrap.dedent(
                        f'''\
                        CALLS={shlex.quote(str(calls))}
                        FAIL_STAGE=""
                        record_stage() {{ printf '%s\\n' "$1" >> "$CALLS"; }}
                        just() {{
                          case "$*" in
                            *browser-contract-test*) stage=browser ;;
                            *html-diagram-consumer-check*) stage=diagram ;;
                            *html-dynamic-fidelity*) stage=dynamic ;;
                            *html-input-state-fidelity*) stage=input ;;
                            *) stage=unknown ;;
                          esac
                          record_stage "$stage"
                          [[ "$stage" == "$FAIL_STAGE" ]] && return 1
                          return 0
                        }}
                        bun() {{
                          case "$*" in
                            *visual-state-fidelity_test.ts*|*interim-feature-results_test.ts*) stage=feature ;;
                            *fidelity-suite.ts*) stage=fidelity ;;
                            *reports-final*) stage=reports ;;
                            *reports-interim*) stage=reports ;;
                            *) stage=unknown ;;
                          esac
                          record_stage "$stage"
                          [[ "$stage" == "$FAIL_STAGE" ]] && return 1
                          return 0
                        }}
                        '''
                    )
                    expected = [
                        "browser",
                        "feature",
                        "fidelity",
                        "diagram",
                        "dynamic",
                        "input",
                        "reports",
                    ]
                    for failure in ["", *expected]:
                        with self.subTest(failure=failure):
                            script = f'{stub}FAIL_STAGE={shlex.quote(failure)}\n{command}'
                            completed = subprocess.run(
                                ["bash", "-c", script],
                                cwd=root,
                                capture_output=True,
                                text=True,
                            )
                            expected_exit = 0 if not failure else 1
                            self.assertEqual(
                                completed.returncode, expected_exit, completed.stderr
                            )
                            observed = calls.read_text(encoding="utf-8").splitlines()
                            calls.write_text("", encoding="utf-8")
                            if not failure:
                                expected_calls = expected
                            else:
                                failure_index = expected.index(failure)
                                expected_calls = expected[: max(4, failure_index + 1)]
                            self.assertEqual(observed, expected_calls)

    def test_html_fidelity_uses_release_profile_for_probe_and_libtest_inputs(
        self,
    ) -> None:
        cases = (
            (
                "html-fidelity-probe-build",
                ("--retain-build-profile release", "build --release"),
            ),
            (
                "html-fidelity-check",
                ("cargo test --release -p katana-render-runtime", "toc-test-build.jsonl"),
            ),
            (
                "html-fidelity-interim-check",
                ("cargo test --release -p katana-render-runtime", "toc-test-build.jsonl"),
            ),
            (
                "html-dynamic-fidelity-check",
                ("cargo test --release -p katana-render-runtime", "dynamic/test-build.jsonl"),
            ),
            (
                "html-dynamic-fidelity-interim-check",
                (
                    "cargo test --release -p katana-render-runtime",
                    "dynamic-interim/test-build.jsonl",
                ),
            ),
            (
                "html-input-state-fidelity-check",
                (
                    "cargo test --release -p katana-render-runtime",
                    "input-state/test-build.jsonl",
                ),
            ),
            (
                "html-input-state-fidelity-interim-check",
                (
                    "cargo test --release -p katana-render-runtime",
                    "input-state-interim/test-build.jsonl",
                ),
            ),
            (
                "html-performance-catalog-captures",
                ("cargo test --release -p katana-render-runtime", "test-build.jsonl"),
                ("tmp/html-performance-stage90/contract",),
            ),
        )
        self.assertIn("target/release/examples/html_fidelity_probe", self.justfile)
        for case in cases:
            recipe, required, *arguments = case
            with self.subTest(recipe=recipe):
                dry_run = self.just_dry_run(
                    recipe,
                    *(arguments[0] if arguments else ()),
                )
                for fragment in required:
                    self.assertIn(fragment, dry_run)

    def test_html_release_fidelity_check_dispatches_the_policy_selected_recipe(
        self,
    ) -> None:
        root = Path(__file__).parents[2]
        recipe_command = self.just_dry_run("html-release-fidelity-check").strip()
        self.assertEqual(
            recipe_command,
            'recipe="$(python3 scripts/release/html_fidelity_policy.py --format recipe)" '
            '&& just "$recipe"',
        )

        selection = subprocess.run(
            [
                "python3",
                "scripts/release/html_fidelity_policy.py",
                "--format",
                "recipe",
            ],
            cwd=root,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        self.assertIn(selection, ("html-fidelity-check", "html-fidelity-interim-check"))
        policy_path = (root / "scripts/release/html_fidelity_policy.py").resolve()
        isolated_command = recipe_command.replace(
            "python3 scripts/release/html_fidelity_policy.py",
            f"python3 {shlex.quote(str(policy_path))}",
        )
        with tempfile.TemporaryDirectory() as temporary_directory:
            isolated_justfile = Path(temporary_directory) / "Justfile"
            isolated_justfile.write_text(
                "\n".join(
                    (
                        "html-release-fidelity-check:",
                        f"    @{isolated_command}",
                        "html-fidelity-check:",
                        "    @echo html-fidelity-check",
                        "html-fidelity-interim-check:",
                        "    @echo html-fidelity-interim-check",
                        "",
                    )
                ),
                encoding="utf-8",
            )
            dispatched = subprocess.run(
                [
                    "just",
                    "--justfile",
                    str(isolated_justfile),
                    "--working-directory",
                    temporary_directory,
                    "html-release-fidelity-check",
                ],
                cwd=temporary_directory,
                capture_output=True,
                text=True,
            )
        self.assertEqual(dispatched.returncode, 0, dispatched.stderr)
        self.assertEqual(dispatched.stdout.strip(), selection)

        target = self.just_dry_run(selection)
        phase, threshold = (
            ("interim", "90")
            if selection == "html-fidelity-interim-check"
            else ("final", "99")
        )
        self.assertIn(f"--phase {phase} --min-score {threshold}", target)
        self.assertEqual(target.count("--min-score"), 1)

    @staticmethod
    def just_dry_run(*arguments: str) -> str:
        result = subprocess.run(
            ["just", "--dry-run", *arguments],
            cwd=Path(__file__).parents[2],
            capture_output=True,
            text=True,
            check=True,
        )
        return result.stdout + result.stderr

    def test_html_diagram_consumers_keep_both_phase_thresholds_and_independent_cases(
        self,
    ) -> None:
        for phase, threshold, parent, output in (
            (
                "final",
                "99",
                "html-fidelity-check",
                "tmp/html-fidelity/diagram-consumer",
            ),
            (
                "interim",
                "90",
                "html-fidelity-interim-check",
                "tmp/html-fidelity/interim/diagram-consumer",
            ),
        ):
            with self.subTest(phase=phase):
                self.assertIn(
                    f"just html-diagram-consumer-check {phase} {output}",
                    self.just_dry_run(parent),
                )
                lexer = shlex.shlex(
                    self.just_dry_run("html-diagram-consumer-check", phase, output),
                    posix=True,
                    punctuation_chars=True,
                )
                lexer.whitespace_split = True
                command = list(lexer)
                for option, expected in (
                    ("--phase", phase),
                    ("--min-score", threshold),
                ):
                    self.assertEqual(command[command.index(option) + 1], expected)
                for fixture in ("mermaid-class-diagram", "drawio-class-diagram"):
                    self.assertIn(fixture, command)

    def test_ubuntu_coverage_owns_unit_tests_without_manual_cache_purges(self) -> None:
        unit_step = self.workflow_step(self.ci, "Run tests")
        self.assertIn("if: matrix.os != 'ubuntu-latest'", unit_step)
        self.assertIn("run: just unit-test", unit_step)
        coverage_step = self.workflow_step(self.ci, "Run coverage")
        self.assertIn("if: matrix.os == 'ubuntu-latest'", coverage_step)
        self.assertIn("run: just coverage", coverage_step)
        self.assertIn("timeout-minutes: 45", coverage_step)

        self.assertIn("uses: Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0", self.ci)
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
            self.workflow_step(
                self.ci, "Record Ubuntu build disk usage after coverage"
            ),
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

    def test_ubuntu_repairs_only_a_missing_cached_v8_archive_before_first_cargo_link(
        self,
    ) -> None:
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

    def test_ubuntu_v8_repair_cleans_only_targets_with_missing_or_empty_archives(
        self,
    ) -> None:
        script = self.workflow_script(self.ci, "Repair cached V8 archive on Ubuntu")
        cases: tuple[tuple[str | None, str | None, list[str]], ...] = (
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
                        archive = root / target_dir / "debug/gn_out/obj/librusty_v8.a"
                        if target_dir != missing_target:
                            archive.parent.mkdir(parents=True, exist_ok=True)
                            archive.write_bytes(
                                b"" if target_dir == empty_target else b"archive"
                            )

                    binary_dir = root / "bin"
                    binary_dir.mkdir()
                    cargo = binary_dir / "cargo"
                    cargo.write_text(
                        '#!/bin/sh\nprintf \'%s\\n\' "$*" >> "$CARGO_CALL_LOG"\n',
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

    def test_coverage_identity_environment_is_exported_from_shared_justfile(self) -> None:
        definitions = [
            line.strip()
            for line in self.justfile.splitlines()
            if line.strip().startswith((
                "export TEST_THREADS :=",
                "export CARGO_BUILD_JOBS :=",
                "export CARGO :=",
            ))
        ]
        self.assertEqual(
            definitions,
            [
                'export TEST_THREADS := env_var_or_default("TEST_THREADS", "2")',
                "export CARGO_BUILD_JOBS := JOBS",
                'export CARGO := env_var_or_default("CARGO", RTK_PROXY_CMD + "cargo")',
            ],
        )

    def test_coverage_delegates_root_collection_without_wiping_build_cache(
        self,
    ) -> None:
        root = Path(__file__).parents[2]
        dumped = subprocess.run(
            ["just", "--dump", "--dump-format", "json"],
            cwd=root,
            capture_output=True,
            text=True,
            check=True,
        )
        document: object = json.loads(dumped.stdout)
        assert isinstance(document, dict)
        recipes: object = document.get("recipes")
        assert isinstance(recipes, dict)
        wrapper: object = recipes.get("coverage")
        assert isinstance(wrapper, dict)
        self.assertEqual(
            wrapper.get("body"),
            [
                ["just performance-harness-coverage"],
                ["just root-test-and-coverage"],
                ["just coverage-result-check"],
            ],
        )
        expanded = subprocess.run(
            ["just", "--dry-run", "root-test-and-coverage"],
            cwd=root,
            env={**os.environ, "CARGO": "cargo", "TEST_THREADS": "2"},
            capture_output=True,
            text=True,
            check=True,
        )
        recipe_commands = (expanded.stdout + expanded.stderr).splitlines()
        parsed_commands = [shlex.split(command) for command in recipe_commands]
        delegated = [
            tokens for tokens in parsed_commands
            if "scripts/release/native_nextest_root_coverage.py" in tokens
        ]
        self.assertEqual(len(delegated), 1)
        producer = delegated[0]
        script_index = producer.index("scripts/release/native_nextest_root_coverage.py")
        self.assertEqual(producer[script_index + 1:], ["--repo", str(root)])
        self.assertIn("CARGO=cargo", producer[:script_index])
        self.assertIn("TEST_THREADS=2", producer[:script_index])
        self.assertFalse(any("llvm-cov" in tokens for tokens in parsed_commands))

        aggregate = subprocess.run(
            ["just", "--dry-run", "coverage-result-check"],
            cwd=root,
            capture_output=True,
            text=True,
            check=True,
        )
        aggregate_commands = [
            shlex.split(command)
            for command in (aggregate.stdout + aggregate.stderr).splitlines()
            if command.strip()
        ]
        self.assertEqual(
            aggregate_commands,
            [
                [
                    "python3",
                    "scripts/release/native_coverage_local_aggregate.py",
                    "--repo",
                    str(root),
                ]
            ],
        )
        aggregate_source = (
            root / "scripts/release/native_coverage_local_aggregate.py"
        ).read_text(encoding="utf-8")
        self.assertIn('failure == "coverage-policy"', aggregate_source)
        self.assertIn('valid": valid', aggregate_source)

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

    def test_check_dry_run_uses_two_threads_by_default_and_preserves_override(
        self,
    ) -> None:
        root = Path(__file__).parents[2]
        cases: tuple[tuple[str | None, str], ...] = (
            (None, "2"),
            ("1", "1"),
            ("3", "3"),
        )
        for supplied, expected in cases:
            with self.subTest(supplied=supplied):
                environment = os.environ.copy()
                if supplied is None:
                    environment.pop("TEST_THREADS", None)
                else:
                    environment["TEST_THREADS"] = supplied
                completed = subprocess.run(
                    ["just", "--dry-run", "check"],
                    cwd=root,
                    env=environment,
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(completed.returncode, 0, completed.stderr)
                output = completed.stdout + completed.stderr
                self.assertIn(f"TEST_THREADS='{expected}'", output)

    def test_native_nextest_diagnostic_supports_scoped_filter_and_prerequisites(
        self,
    ) -> None:
        root = Path(__file__).parents[2]
        filter_cases: tuple[tuple[str, tuple[str, ...]], ...] = (
            ("", ("--test-threads=2",)),
            ("class diagram [ja]", ("--test-threads=2", "-E", "class diagram [ja]")),
        )
        job_cases: tuple[tuple[str | None, str], ...] = ((None, "2"), ("3", "3"))
        for filter_value, expected_suffix in filter_cases:
            for supplied_jobs, expected_jobs in job_cases:
                with self.subTest(filter=filter_value, jobs=supplied_jobs):
                    environment = os.environ.copy()
                    if supplied_jobs is None:
                        environment.pop("JOBS", None)
                    else:
                        environment["JOBS"] = supplied_jobs
                    environment["TEST_THREADS"] = "2"
                    completed = subprocess.run(
                        ["just", "--dry-run", "native-nextest-diagnostic", filter_value],
                        cwd=root,
                        env=environment,
                        capture_output=True,
                        text=True,
                        check=True,
                    )
                    commands = [
                        shlex.split(line)
                        for line in (completed.stdout + completed.stderr).splitlines()
                        if "llvm-cov" in line and "nextest" in line
                    ]
                    self.assertEqual(len(commands), 1)
                    command = commands[0]
                    self.assertIn(f"CARGO_BUILD_JOBS={expected_jobs}", command)
                    self.assertIn("--workspace", command)
                    self.assertIn("--all-targets", command)
                    self.assertIn("--all-features", command)
                    self.assertIn("--no-report", command)
                    for token in expected_suffix:
                        self.assertIn(token, command)
                    if filter_value:
                        self.assertLess(command.index("-E"), len(command) - 1)
                        self.assertEqual(command[command.index("-E") + 1], filter_value)
                    else:
                        self.assertNotIn("-E", command)
                    rendered = completed.stdout + completed.stderr
                    self.assertIn("plantuml-lgpl", rendered)
                    self.assertIn("bun run runtime-bundle:check", rendered)

    def test_automation_contract_tests_fail_fast_with_all_discovery_scopes(
        self,
    ) -> None:
        start = self.justfile.index("automation-contract-test:")
        end = self.justfile.index("\n\n", start)
        recipe = self.justfile[start:end]
        commands = [
            shlex.split(line.strip())
            for line in recipe.splitlines()[1:]
            if line.strip()
        ]
        expected = [
            [
                "python3",
                "-m",
                "unittest",
                "discover",
                "-s",
                "scripts/hooks",
                "-p",
                "*_test.py",
                "--failfast",
            ],
            [
                "{{HTML_PERFORMANCE_SCIPY_ENV}}",
                "uv",
                "run",
                "--isolated",
                "--with-requirements",
                "{{quote(HTML_PERFORMANCE_PYTHON_REQUIREMENTS)}}",
                "python",
                "scripts/release/run_html_performance.py",
                "test",
            ],
            [
                "python3",
                "-m",
                "unittest",
                "discover",
                "-s",
                "scripts/quality",
                "-p",
                "*_test.py",
                "--failfast",
            ],
            [
                "python3",
                "-m",
                "unittest",
                "discover",
                "-s",
                "scripts/mermaid",
                "-p",
                "*_test.py",
                "--failfast",
            ],
            [
                "python3",
                "-m",
                "unittest",
                "discover",
                "-s",
                "scripts/validation-timing",
                "-p",
                "test_measure.py",
                "--failfast",
            ],
        ]
        self.assertEqual(commands, expected)

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

    def test_release_checks_install_the_bun_resolver_before_freshness_validation(
        self,
    ) -> None:
        for job in (
            self.release[
                self.release.index("  release-context:") : self.release.index(
                    "  release:\n"
                )
            ],
            self.release[self.release.index("  release:\n") :],
        ):
            self.assertIn("uses: oven-sh/setup-bun@0c5077e51419868618aeaa5fe8019c62421857d6", job)
            self.assertIn("bun-version: 1.4.4", job)
            self.assertIn("run: bun install --frozen-lockfile", job)

    def test_performance_retry_uses_pinned_uv_in_release_source(self) -> None:
        self.assertIn(
            "uses: astral-sh/setup-uv@68559e99cc807c1eb7e9238fd43a15bba460eec2",
            self.retry,
        )
        self.assertIn('version: "0.13.0"', self.retry)
        self.assertIn(
            "--with-requirements release-tools/scripts/release/html-performance-requirements.lock",
            self.retry,
        )
        self.assertIn(
            "python release-tools/scripts/release/run_html_performance.py verify",
            self.retry,
        )
        self.assertIn('--repo "$GITHUB_WORKSPACE/release-source"', self.retry)
        self.assertIn(
            '--evidence-dir "$GITHUB_WORKSPACE/release-source/tmp/html-performance-release-evidence"',
            self.retry,
        )
        self.assertNotIn("release-source/scripts/release/html-performance-requirements.lock", self.retry)
        self.assertNotIn("python release-source/scripts/release/run_html_performance.py", self.retry)

    def test_release_performance_jobs_install_pinned_uv_before_wrapper_recipes(self) -> None:
        release_job = self.release[self.release.index("  release:\n") :]
        for label, workflow in (("release", release_job), ("preflight", self.preflight)):
            with self.subTest(workflow=label):
                self.assertIn(
                    "uses: astral-sh/setup-uv@68559e99cc807c1eb7e9238fd43a15bba460eec2",
                    workflow,
                )
                self.assertIn('version: "0.13.0"', workflow)
                self.assertIn("just html-performance-evidence-acquire", workflow)

    def test_release_target_check_requires_dependency_freshness_before_tagging(
        self,
    ) -> None:
        justfile = (Path(__file__).parents[2] / "Justfile").read_text(encoding="utf-8")
        freshness = justfile.index(
            "python3 scripts/release/verify_dependency_freshness.py"
        )
        target = justfile.index("python3 scripts/release/verify-release-target.py")
        self.assertLess(freshness, target)
        freshness_steps = (
            (
                self.release,
                "Release target check",
                'run: just VERSION="${{ steps.version.outputs.version }}" release-target-check',
            ),
            (
                self.release,
                "Verify release package",
                'run: just VERSION="${{ needs.release-context.outputs.version }}" release-verify',
            ),
            (
                self.preflight,
                "Release check",
                'run: just VERSION="${{ steps.version.outputs.version }}" release-check',
            ),
            (
                self.ci,
                "Release preflight checks",
                'run: just VERSION="${{ steps.version.outputs.version }}" release-preflight-check',
            ),
        )
        for workflow, name, command in freshness_steps:
            step = self.workflow_step(workflow, name)
            self.assertIn(command, step, name)
            self.assertIn("env:\n          GH_TOKEN: ${{ github.token }}", step, name)
        self.assertNotIn("- name: Release quality gate", self.preflight)
        self.assertNotIn("- name: Release-specific verification", self.preflight)

    def test_release_preflight_uses_read_only_permissions_for_api_lookups(self) -> None:
        permissions = self.preflight[
            self.preflight.index("permissions:") : self.preflight.index("\nenv:")
        ]
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
        conditional = (
            "if: github.event_name == 'workflow_run' || inputs.publish_crates == true"
        )
        self.assertGreaterEqual(self.release.count(conditional), 3)
        self.assertIn("while IFS= read -r package; do", self.publisher)
        self.assertIn(
            'publish_if_needed "${package}"\n  wait_for_crate "${package}"',
            self.publisher,
        )
        cleanup_step = self.workflow_step(
            self.release, "Cleanup published release state"
        )
        self.assertNotIn("--delete-remote", cleanup_step)

    def test_retry_rejoins_the_same_publication_completion_path(self) -> None:
        publish = self.retry.index("- name: Publish crates.io from release tag")
        public = self.retry.index("- name: Publish complete GitHub Release")
        cleanup = self.retry.index("- name: Cleanup published release state")
        self.assertLess(publish, public)
        self.assertLess(public, cleanup)
        self.assertIn("contents: write", self.retry)
        retry_publish = self.retry[public:cleanup]
        self.assertIn(
            'gh release edit "${TAG}" --repo "${GITHUB_REPOSITORY}" --draft=false --verify-tag',
            retry_publish,
        )
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
        self.assertIn(
            'required = ["katana-render-runtime", "katana-render-runtime-cli"]',
            self.publisher,
        )
        self.assertIn("if parts >= (0, 4, 23):", self.publisher)
        self.assertIn(
            'required = ["katana-render-runtime-assets", *required]', self.publisher
        )
        self.assertIn("for name in required:", self.publisher)
        self.assertIn(
            'publish_if_needed "${package}"\n  wait_for_crate "${package}"',
            self.publisher,
        )

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
            self.justfile.index("release-verify:") : self.justfile.index(
                "# Verify completed OpenSpec changes"
            )
        ]
        self.assertIn(
            "package -p " + " -p ".join(packages) + " --locked --allow-dirty",
            release_verify,
        )
        self.assertIn(
            "python3 scripts/release/test_packaged_runtime.py", release_verify
        )
        self.assertIn('--version "{{VERSION_BARE}}"', release_verify)
        for package in packages:
            self.assertIn(f"verify-crate-size.sh {package}", release_verify)
        self.assertNotIn("publish -p", release_verify)

    def test_post_merge_release_check_allows_only_a_published_package_prefix(
        self,
    ) -> None:
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

            def run(
                published: list[str],
                *,
                allow_prefix: bool = True,
                registry_failure: list[str] | None = None,
            ) -> subprocess.CompletedProcess[str]:
                environment = os.environ.copy()
                environment.update(
                    {
                        "PATH": f"{cargo_bin}:{environment['PATH']}",
                        "PUBLISHED": ",".join(published),
                        "REGISTRY_FAILURE": ",".join(registry_failure or []),
                    }
                )
                if allow_prefix:
                    environment["ALLOW_PUBLISHED_PREFIX"] = "true"
                else:
                    environment.pop("ALLOW_PUBLISHED_PREFIX", None)
                return subprocess.run(
                    ["bash", str(scripts / guard.name), "v0.4.23"],
                    cwd=root,
                    env=environment,
                    capture_output=True,
                    text=True,
                    check=False,
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

    def test_release_package_verification_propagates_prefix_mode_to_nested_target_check(
        self,
    ) -> None:
        package_verification = self.workflow_step(
            self.release, "Verify release package"
        )
        self.assertIn('ALLOW_PUBLISHED_PREFIX: "true"', package_verification)
        self.assertIn(
            'run: just VERSION="${{ needs.release-context.outputs.version }}" release-verify',
            package_verification,
        )
        self.assertRegex(
            self.justfile,
            r"(?m)^release-verify: html-performance-nonregression-check release-target-check$",
        )

        for workflow, name in (
            (self.preflight, "Release check"),
            (self.ci, "Release preflight checks"),
        ):
            premerge_step = self.workflow_step(workflow, name)
            self.assertNotIn("ALLOW_PUBLISHED_PREFIX", premerge_step)

        # 配布検証の依存recipeでも再試行モードが伝わり、既定の未公開要求が残ることを確認する。
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            scripts = root / "scripts/release"
            scripts.mkdir(parents=True)
            guard = Path(__file__).parent / "assert-crates-not-published.sh"
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
                "import sys\n"
                "package = sys.argv[2].split('@')[0]\n"
                "if package == 'katana-render-runtime-assets': sys.exit(0)\n"
                "print('error: could not find package in registry', file=sys.stderr); sys.exit(101)\n",
                encoding="utf-8",
            )
            cargo.chmod(0o755)
            justfile = root / "Justfile"
            justfile.write_text(
                "release-verify: release-target-check\n\n"
                "release-target-check:\n"
                "\tbash scripts/release/assert-crates-not-published.sh v0.4.23\n",
                encoding="utf-8",
            )
            environment = os.environ.copy()
            environment["PATH"] = f"{cargo_bin}:{environment['PATH']}"
            environment["ALLOW_PUBLISHED_PREFIX"] = "true"
            nested_retry = subprocess.run(
                ["just", "--justfile", str(justfile), "release-verify"],
                cwd=root,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(nested_retry.returncode, 0, nested_retry.stderr)
            environment.pop("ALLOW_PUBLISHED_PREFIX")
            nested_strict = subprocess.run(
                ["just", "--justfile", str(justfile), "release-verify"],
                cwd=root,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(nested_strict.returncode, 0)
            self.assertIn("already published", nested_strict.stderr)

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

    def test_runtime_package_asset_check_rejects_drawio_source_and_compressed_copy(
        self,
    ) -> None:
        start = self.justfile.index("runtime-package-asset-check:")
        end = self.justfile.index(
            "# Verify repository hook and release cleanup contracts", start
        )
        recipe = self.justfile[start:end]
        package_list_line = next(
            line for line in recipe.splitlines() if "runtime_package_files=" in line
        )
        self.assertIn(
            "package -p katana-render-runtime --locked --allow-dirty --list",
            package_list_line,
        )
        reject_line = next(line for line in recipe.splitlines() if "grep -Eq" in line)
        self.assertIn("printf '%s\\n' \"$runtime_package_files\"", reject_line)
        self.assertIn(r"drawio\.min\.js(\.br)?$", reject_line)
        self.assertIn("assets/drawio.min.js.br", recipe)
        self.assertIn("vendor/drawio/{{DRAWIO_JS_VERSION}}/LICENSE", recipe)
        self.assertIn("assets/licenses/drawio/LICENSE", recipe)

        release_start = self.justfile.index("release-verify:")
        release_end = self.justfile.index(
            "# Verify completed OpenSpec changes", release_start
        )
        release_recipe = self.justfile[release_start:release_end]
        self.assertIn('"assets/licenses/drawio/LICENSE"', release_recipe)
        self.assertIn('"vendor/drawio/{{DRAWIO_JS_VERSION}}/LICENSE"', release_recipe)

    def run_plantuml_install(
        self, failures: int, expected_checksum: str, observed_checksum: str
    ) -> tuple[subprocess.CompletedProcess[str], Path, int]:
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
            'if [ -f "$attempts" ]; then count=$(cat "$attempts"); fi\n'
            "count=$((count + 1))\n"
            'printf \'%s\\n\' "$count" > "$attempts"\n'
            f'if [ "$count" -le {failures} ]; then exit 22; fi\n'
            'while [ "$#" -gt 0 ]; do\n'
            '  if [ "$1" = "--output" ]; then printf \'fixture\' > "$2"; exit 0; fi\n'
            "  shift\n"
            "done\n"
            "exit 64\n",
            encoding="utf-8",
        )
        sha256sum = fake_bin / "sha256sum"
        sha256sum.write_text(
            f"#!/usr/bin/env bash\nprintf '%s  %s\\n' '{observed_checksum}' \"$1\"\n",
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
            [
                "just",
                "--set",
                "PLANTUML_JAR_CHECKSUM",
                expected_checksum,
                "plantuml-install",
                "1.2026.8",
                str(output),
            ],
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
            expected_checksum=PLANTUML_FIXTURE_SHA256,
            observed_checksum=PLANTUML_FIXTURE_SHA256,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(attempts, 2)
        self.assertEqual(output.read_text(encoding="utf-8"), "fixture")
        self.assertFalse(output.with_suffix(".jar.tmp").exists())

    def test_plantuml_install_fails_closed_after_bounded_retries(self) -> None:
        completed, output, attempts = self.run_plantuml_install(
            failures=3,
            expected_checksum=PLANTUML_FIXTURE_SHA256,
            observed_checksum=PLANTUML_FIXTURE_SHA256,
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(attempts, 3)
        self.assertFalse(output.exists())
        self.assertFalse(output.with_suffix(".jar.tmp").exists())

    def test_plantuml_install_keeps_checksum_validation_after_recovery(self) -> None:
        completed, output, attempts = self.run_plantuml_install(
            failures=1,
            expected_checksum=PLANTUML_FIXTURE_SHA256,
            observed_checksum="0" * 64,
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(attempts, 2)
        self.assertFalse(output.exists())
        self.assertFalse(output.with_suffix(".jar.tmp").exists())


if __name__ == "__main__":
    unittest.main()
