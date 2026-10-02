from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).parent))

import local_review
from local_review_contract import CHECKS, receipt_payload, validate_receipt
from local_review_state import ReviewError, command, digest, issue_context, source_snapshot, strict_json


def inputs() -> dict:
    return {"source": {"base_sha": "a" * 40, "files": {"example.py": {"sha256": "b" * 64}}},
            "issues": [{"number": 89, "body_sha256": "c" * 64}],
            "model": "gpt-6.1-sol", "reasoning": "high", "schema_sha256": "d" * 64}


def review(value: dict) -> dict:
    evidence = [{"source": "example.py:1", "quote": "assert observed == expected",
                 "observation": "the assertion agrees with the requirement"}]
    return {"input_sha256": digest(value), "verdict": "PASS", "summary": "reviewed",
            "full_quality_gate": "not_run", "pending": ["full_quality_gate", "native_ci", "protected_merge"],
            "checks": {key: {"result": "verified", "reason": "verified against source",
                              "evidence": copy.deepcopy(evidence)} for key in CHECKS},
            "issues": [{"number": 89, "body_sha256": "c" * 64, "result": "verified",
                        "evidence": copy.deepcopy(evidence)}], "findings": []}


class ReceiptContractTest(unittest.TestCase):
    def test_changed_source_invalidates_old_receipt(self) -> None:
        before = inputs()
        receipt = receipt_payload(before, review(before))
        after = copy.deepcopy(before)
        after["source"]["files"]["example.py"]["sha256"] = "e" * 64
        with self.assertRaisesRegex(ReviewError, "another input|stale"):
            validate_receipt(receipt, after)

    def test_receipt_integrity_rejects_modified_verdict(self) -> None:
        value = inputs()
        receipt = receipt_payload(value, review(value))
        receipt["review"]["summary"] = "changed after review"
        with self.assertRaisesRegex(ReviewError, "integrity"):
            validate_receipt(receipt, value)

    def test_changed_base_issue_model_schema_or_requirements_invalidates_receipt(self) -> None:
        value = inputs()
        receipt = receipt_payload(value, review(value))
        for key, replacement in [("model", "other"), ("reasoning", "low"),
                                 ("schema_sha256", "f" * 64), ("requirements", "changed")]:
            with self.subTest(key=key):
                changed = {**value, key: replacement}
                with self.assertRaisesRegex(ReviewError, "another input"):
                    validate_receipt(receipt, changed)
        for key in ("source", "issues"):
            changed = copy.deepcopy(value)
            if key == "source":
                changed[key]["base_sha"] = "f" * 40
            else:
                changed[key][0]["body_sha256"] = "f" * 64
            with self.assertRaisesRegex(ReviewError, "another input"):
                validate_receipt(receipt, changed)

    def test_unverified_or_missing_assertion_oracle_rejects_pass(self) -> None:
        value = inputs()
        for missing in (True, False):
            result = review(value)
            if missing:
                del result["checks"]["specification_and_assertion_oracle"]
            else:
                result["checks"]["specification_and_assertion_oracle"]["result"] = "blocked"
            with self.assertRaises(ReviewError):
                validate_receipt(receipt_payload(value, result), value)

    def test_priority_triage_blocks_relevant_findings_and_requires_acceptance(self) -> None:
        value = inputs()
        for priority in ("P0", "P1", "P2", "P3"):
            result = review(value)
            result["findings"] = [{"priority": priority, "blocking": True, "status": "open",
                                  "title": "wrong assertion", "reason": "violates requirement",
                                  "evidence": result["checks"][CHECKS[0]]["evidence"]}]
            with self.subTest(priority=priority), self.assertRaisesRegex(ReviewError, "blocking"):
                validate_receipt(receipt_payload(value, result), value)
        result["findings"][0].update(priority="P2", blocking=False, status="accepted",
                                       reason="unrelated to the requirement and compatibility")
        validate_receipt(receipt_payload(value, result), value)
        result["findings"][0]["reason"] = ""
        with self.assertRaises(ReviewError):
            validate_receipt(receipt_payload(value, result), value)

    def test_pending_later_gates_are_allowed_but_not_a_fake_quality_pass(self) -> None:
        value = inputs()
        validate_receipt(receipt_payload(value, review(value)), value)
        result = review(value)
        result["full_quality_gate"] = "passed"
        with self.assertRaises(ReviewError):
            validate_receipt(receipt_payload(value, result), value)

    def test_json_duplicate_fields_are_rejected(self) -> None:
        with self.assertRaisesRegex(ReviewError, "duplicate"):
            strict_json('{"verdict":"FAIL","verdict":"PASS"}')


class GitSnapshotTest(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        command(["git", "init", "--quiet"], self.root)
        (self.root / ".gitignore").write_text("tmp/\n")
        (self.root / "example.py").write_text("original\n")
        self.commit("initial")
        self.base = command(["git", "rev-parse", "HEAD"], self.root).strip()

    def commit(self, message: str) -> None:
        command(["git", "add", "-A"], self.root)
        command(["git", "-c", "user.name=Review fixture", "-c", "user.email=review@example.invalid",
                 "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", message], self.root)

    def test_staging_and_committing_same_bytes_preserves_source_snapshot(self) -> None:
        (self.root / "example.py").write_text("changed\n")
        (self.root / "new.py").write_text("new\n")
        before = source_snapshot(self.root, self.base)
        command(["git", "add", "-A"], self.root)
        self.assertEqual(before, source_snapshot(self.root, self.base))
        self.commit("changes")
        self.assertEqual(before, source_snapshot(self.root, self.base))

    def test_untracked_deleted_mode_and_hidden_staged_changes_are_bound(self) -> None:
        initial = source_snapshot(self.root, self.base)
        path = self.root / "example.py"
        path.chmod(0o755)
        self.assertNotEqual(initial, source_snapshot(self.root, self.base))
        path.write_text("staged\n")
        command(["git", "add", "example.py"], self.root)
        path.write_text("unstaged\n")
        before = source_snapshot(self.root, self.base)
        self.assertTrue(before["index_overrides"])
        path.write_text("different staged\n")
        command(["git", "add", "example.py"], self.root)
        path.write_text("unstaged\n")
        self.assertNotEqual(before, source_snapshot(self.root, self.base))
        path.unlink()
        self.assertEqual(source_snapshot(self.root, self.base)["files"]["example.py"], {"kind": "deleted"})

    def test_committed_deletion_retains_baseline_tombstone(self) -> None:
        (self.root / "example.py").unlink()
        before = source_snapshot(self.root, self.base)
        self.commit("delete")
        self.assertEqual(before, source_snapshot(self.root, self.base))

    def test_staged_deletion_with_restored_untracked_file_is_not_lost(self) -> None:
        command(["git", "rm", "example.py"], self.root)
        (self.root / "example.py").write_text("restored\n")
        state = source_snapshot(self.root, self.base)
        self.assertIn("deleted\texample.py", state["index_overrides"])


class DriverContractTest(unittest.TestCase):
    def test_actual_just_overrides_reach_review_and_quality_runner(self) -> None:
        repository = Path(__file__).resolve().parents[2]
        names = ("COVERAGE_MIN_LINES", "COVERAGE_MAX_UNCOVERED_LINES", "TEST_THREADS",
                 "RUSTFLAGS", "CARGO", "JOBS", "CHECK_JOBS")
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {"PATH": os.environ["PATH"]}, clear=True):
            root = Path(temporary).resolve()
            (root / "Justfile").write_text((repository / "Justfile").read_text())
            (root / "Cargo.toml").write_text('[package]\nversion = "0.4.22"\n')
            hooks = root / "scripts" / "hooks"
            hooks.mkdir(parents=True)
            cache_script = root / "scripts" / "plantuml" / "cache-dir.sh"
            cache_script.parent.mkdir()
            cache_script.write_text((repository / "scripts/plantuml/cache-dir.sh").read_text())
            common = f"import json,sys\nfrom pathlib import Path\nsys.path.insert(0, {str(repository / 'scripts/hooks')!r})\nfrom local_review_state import gate_configuration\n"
            (hooks / "local_review.py").write_text(common + "Path('review.json').write_text(json.dumps(gate_configuration(Path.cwd())))\n")
            (hooks / "run_parallel_checks.py").write_text(common + "Path('runner.json').write_text(json.dumps({'config':gate_configuration(Path.cwd()),'jobs':sys.argv[-1]}))\n")
            def run(overrides):
                command(["just", "--justfile", str(root / "Justfile"), *overrides, "check"], root)
                reviewed = json.loads((root / "review.json").read_text())
                executed = json.loads((root / "runner.json").read_text())
                self.assertEqual(reviewed, executed["config"])
                self.assertEqual(reviewed["CHECK_JOBS"], executed["jobs"])
                return reviewed
            baseline = run([])
            self.assertEqual(baseline, local_review.gate_configuration(root))
            explicit = [argument for name in names if name != "RUSTFLAGS" for argument in ("--set", name, baseline[name])]
            explicit.append(f"RUSTFLAGS={baseline['RUSTFLAGS']}")
            self.assertEqual(baseline, run(explicit))
            for name, changed in [("CHECK_JOBS", "1"), ("TEST_THREADS", "2"), ("CARGO", "cargo --offline"),
                                  ("JOBS", "4"), ("RUSTFLAGS", "-D warnings -C opt-level=1"),
                                  ("COVERAGE_MIN_LINES", "100.0"), ("COVERAGE_MAX_UNCOVERED_LINES", "00")]:
                with self.subTest(name=name):
                    override = [f"RUSTFLAGS={changed}"] if name == "RUSTFLAGS" else ["--set", name, changed]
                    value = run(override)
                    self.assertEqual(value[name], changed)
                    self.assertNotEqual(digest(baseline), digest(value))

    def test_coverage_target_paths_are_effective_and_normalized(self) -> None:
        from local_review_state import gate_configuration
        root = Path(__file__).resolve().parents[2]
        with patch("local_review_state.command", return_value="value\n"), patch.dict(os.environ, {}, clear=True):
            before = gate_configuration(root)
            self.assertEqual(before["CARGO_LLVM_COV_TARGET_DIR"], str(root / "target/llvm-cov-target"))
            with patch.dict(os.environ, {"CARGO_LLVM_COV_BUILD_DIR": "tmp/unused-without-target"}):
                self.assertEqual(before, gate_configuration(root))
            with patch.dict(os.environ, {"CARGO_LLVM_COV_TARGET_DIR": "target/llvm-cov-target"}):
                self.assertEqual(before, gate_configuration(root))
            with patch.dict(os.environ, {"CARGO_LLVM_COV_TARGET_DIR": "tmp/coverage-target"}):
                changed = gate_configuration(root)
                self.assertNotEqual(digest(before), digest(changed))
                with patch.dict(os.environ, {"CARGO_LLVM_COV_TARGET_DIR": str(root / "tmp/coverage-target")}):
                    self.assertEqual(changed, gate_configuration(root))
                with patch.dict(os.environ, {"CARGO_LLVM_COV_BUILD_DIR": "tmp/coverage-build"}):
                    self.assertNotEqual(changed, gate_configuration(root))
                    self.assertEqual(gate_configuration(root)["CARGO_LLVM_COV_BUILD_DIR"], str(root / "tmp/coverage-build"))

    def test_effective_gate_settings_invalidate_receipt_and_defaults_reuse(self) -> None:
        root = Path(__file__).resolve().parents[2]
        args = SimpleNamespace(base="origin/master", requirements=None)
        with patch.object(local_review, "source_snapshot", return_value=inputs()["source"]), patch.object(local_review, "issue_context", return_value=inputs()["issues"]):
            with patch.dict(os.environ, {"PATH": os.environ["PATH"]}, clear=True):
                baseline = local_review.build_inputs(root, args, [89])
            for name, changed in [("COVERAGE_MIN_LINES", "95"), ("COVERAGE_MAX_UNCOVERED_LINES", "2"),
                                  ("TEST_THREADS", "8"), ("RUSTFLAGS", ""), ("CARGO", "cargo --offline"),
                                  ("CARGO_BUILD_TARGET", "aarch64-apple-darwin")]:
                with self.subTest(name=name), patch.dict(os.environ, {"PATH": os.environ["PATH"], name: changed}, clear=True):
                    updated = local_review.build_inputs(root, args, [89])
                    self.assertNotEqual(baseline, updated)
                    with self.assertRaisesRegex(ReviewError, "another input"):
                        validate_receipt(receipt_payload(baseline, review(baseline)), updated)
            defaults = {"PATH": os.environ["PATH"], "COVERAGE_MIN_LINES": "100",
                        "COVERAGE_MAX_UNCOVERED_LINES": "0", "TEST_THREADS": "1", "RUSTFLAGS": "-D warnings",
                        "GITHUB_TOKEN": "secret-not-an-input"}
            with patch.dict(os.environ, defaults, clear=True):
                self.assertEqual(baseline, local_review.build_inputs(root, args, [89]))
            self.assertNotIn("GITHUB_TOKEN", json.dumps(baseline))

    def test_requirements_are_retained_for_same_task_and_rehashed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {}, clear=True):
            root = Path(temporary).resolve()
            requirement = root / "requirements.md"
            requirement.write_text("native behavior\n")
            value = inputs()
            value["requirements"] = local_review.requirements_context(root, "requirements.md")
            receipt = root / "receipt.json"
            receipt.write_text(json.dumps(receipt_payload(value, review(value))))
            args = SimpleNamespace(requirements=None)
            selected = local_review.requirements_path(root, args, receipt, [89])
            self.assertEqual(selected, "requirements.md")
            self.assertEqual(value["requirements"], local_review.requirements_context(root, selected))
            requirement.write_text("changed native requirement\n")
            self.assertNotEqual(value["requirements"], local_review.requirements_context(root, selected))
            self.assertIsNone(local_review.requirements_path(root, args, receipt, [120]))
            with patch.dict(os.environ, {"REVIEW_REQUIREMENTS": "environment.md"}):
                self.assertEqual(local_review.requirements_path(root, args, receipt, [89]), "environment.md")
                args.requirements = "explicit.md"
                self.assertEqual(local_review.requirements_path(root, args, receipt, [89]), "explicit.md")
            args.requirements = None
            requirement.unlink()
            with self.assertRaisesRegex(ReviewError, "requirements"):
                local_review.requirements_path(root, args, receipt, [89])
            value["requirements"] = {"content": "lost path", "sha256": "a" * 64}
            receipt.write_text(json.dumps(receipt_payload(value, review(value))))
            with self.assertRaisesRegex(ReviewError, "requirements"):
                local_review.requirements_path(root, args, receipt, [89])

    def test_standard_check_reuses_required_input_and_rejects_changed_requirement(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {}, clear=True):
            root = Path(temporary).resolve()
            requirement = root / "requirements.md"
            requirement.write_text("native requirement\n")
            path = root / "tmp" / "receipt.json"
            path.parent.mkdir()
            args = SimpleNamespace(issue=[89], base="origin/master", requirements="requirements.md",
                                   receipt="tmp/receipt.json", print_input=False, check_receipt=True)
            with patch.object(local_review, "repository_root", return_value=root), patch.object(local_review, "source_snapshot", return_value=inputs()["source"]), patch.object(local_review, "issue_context", return_value=inputs()["issues"]), patch.object(local_review, "gate_configuration", return_value={"TEST_THREADS": "1"}), patch.object(local_review, "invoke_review") as invoke:
                value = local_review.build_inputs(root, args, [89])
                result = review(value)
                path.write_text(json.dumps(receipt_payload(value, result)))
                path.with_suffix(".review.json").write_text(json.dumps(result))
                args.requirements = None
                self.assertEqual(local_review.run(args), 0)
                self.assertEqual(args.requirements, "requirements.md")
                invoke.assert_not_called()
                requirement.write_text("different requirement\n")
                args.requirements = None
                with self.assertRaisesRegex(ReviewError, "receipt rejected"):
                    local_review.run(args)
                invoke.assert_not_called()

    def test_ci_and_recursion_do_not_start_codex(self) -> None:
        args = SimpleNamespace()
        for flag in ("CI", "GITHUB_ACTIONS"):
            with patch.dict(os.environ, {flag: "true"}, clear=True), patch.object(local_review, "invoke_review") as run:
                self.assertEqual(local_review.run(args), 0)
                run.assert_not_called()
        with patch.dict(os.environ, {"KRR_LOCAL_REVIEW_ACTIVE": "1"}, clear=True):
            with self.assertRaisesRegex(ReviewError, "nested"):
                local_review.run(args)

    def test_failed_initial_review_preserves_requirements_and_issue_without_receipt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {}, clear=True):
            root = Path(temporary).resolve()
            requirement = root / "requirements.md"
            requirement.write_text("required native semantics\n")
            value = inputs()
            value["requirements"] = local_review.requirements_context(root, "requirements.md")
            result = review(value)
            result["verdict"] = "FAIL"
            child = root / "codex-test"
            child.mkdir()
            def returned(arguments, *_options):
                Path(arguments[arguments.index("--output-last-message") + 1]).write_text(json.dumps(result))
                return 0
            with patch.object(local_review, "compact_input", return_value={}), patch.object(local_review, "run_review_process", side_effect=returned):
                with self.assertRaisesRegex(ReviewError, "findings retained"):
                    local_review.invoke_review(root, child, value)
            receipt = root / "receipt.json"
            self.assertFalse(receipt.exists())
            args = SimpleNamespace(issue=[], base="origin/master", requirements=None)
            self.assertEqual(local_review.requirements_path(root, args, receipt, [89]), "requirements.md")
            with patch.object(local_review, "command", return_value=""):
                self.assertEqual(local_review.issue_numbers(root, args, receipt), [89])
            with patch.object(local_review, "command", return_value="Refs #120"):
                self.assertEqual(local_review.issue_numbers(root, args, receipt), [120])
            self.assertIsNone(local_review.requirements_path(root, args, receipt, [120]))
            requirement.unlink()
            with self.assertRaisesRegex(ReviewError, "requirements"):
                local_review.requirements_path(root, args, receipt, [89])
            value["requirements"]["path"] = "different.md"
            (root / "last-input.json").write_text(json.dumps(value))
            with self.assertRaisesRegex(ReviewError, "retained input"):
                local_review.requirements_path(root, args, receipt, [89])
            self.assertIsNone(local_review.requirements_path(root, args, receipt, [120]))

    def test_failed_findings_survive_temporary_review_directory(self) -> None:
        value = inputs()
        result = review(value)
        result["verdict"] = "FAIL"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            child = root / "codex-test"
            child.mkdir()

            def returned(arguments, *_options):
                Path(arguments[arguments.index("--output-last-message") + 1]).write_text(json.dumps(result))
                return 0

            with patch.object(local_review, "compact_input", return_value={}), patch.object(local_review, "run_review_process", side_effect=returned):
                with self.assertRaisesRegex(ReviewError, "findings retained"):
                    local_review.invoke_review(root, child, value)
            self.assertEqual(json.loads((root / "last-review.json").read_text()), result)
            self.assertEqual(json.loads((root / "last-input.json").read_text()), value)

    def test_cli_error_does_not_create_a_receipt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            child = root / "codex-test"
            child.mkdir()
            with patch.object(local_review, "compact_input", return_value={}), patch.object(local_review, "run_review_process", return_value=2):
                with self.assertRaisesRegex(ReviewError, "exit 2"):
                    local_review.invoke_review(root, child, inputs())
            self.assertFalse((root / "receipt.json").exists())


    def test_prompt_scope_and_finite_review_budget_are_input_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            args = SimpleNamespace(base="origin/master", requirements=None)
            with patch.object(local_review, "source_snapshot", return_value=inputs()["source"]), patch.object(local_review, "issue_context", return_value=inputs()["issues"]), patch.object(local_review, "gate_configuration", return_value={}):
                before = local_review.build_inputs(root, args, [89])
                with patch.object(local_review, "REVIEW_TIMEOUT_SECONDS", 1801):
                    after = local_review.build_inputs(root, args, [89])
                self.assertNotEqual(before["prompt_sha256"], after["prompt_sha256"])
                with self.assertRaisesRegex(ReviewError, "another input|stale"):
                    validate_receipt(receipt_payload(before, review(before)), after)
            prompt = local_review.review_prompt()
            self.assertLess(local_review.REVIEW_ANALYSIS_SECONDS, local_review.REVIEW_TIMEOUT_SECONDS)
            self.assertIn("blocked/FAIL", prompt)
            self.assertIn("runtime 60秒/close 5秒", prompt)
            self.assertTrue(all(check in prompt for check in CHECKS))

    def test_review_request_carries_deadline_and_timeout_rejects_receipt(self) -> None:
        value = inputs()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            receipt = root / "receipt.json"
            args = SimpleNamespace()
            def timed_out(arguments, _root, _environment, prompt, diagnostic):
                payload = json.loads(prompt.split("固定入力:\n", 1)[1])
                budget = payload["review_budget"]
                self.assertEqual(budget["process_timeout_seconds"], 1800)
                self.assertEqual(budget["analysis_seconds"], 1500)
                self.assertIn("+00:00", budget["analysis_deadline_utc"])
                diagnostic.write_text("partial review; not a PASS")
                raise subprocess.TimeoutExpired(arguments, 1800)
            with patch.object(local_review, "compact_input", return_value={"input_sha256": digest(value)}), patch.object(local_review, "run_review_process", side_effect=timed_out):
                with self.assertRaisesRegex(ReviewError, "exceeded 1800s; no receipt accepted; inspect"):
                    local_review.obtain_receipt(root, args, receipt, value, [89])
            self.assertFalse(receipt.exists())
            self.assertFalse(receipt.with_suffix(".review.json").exists())
            self.assertFalse((root / "review.lock").exists())
            self.assertEqual(json.loads((root / "last-input.json").read_text()), value)
            self.assertEqual((root / "last-codex-stderr.log").read_text(), "partial review; not a PASS")

    @unittest.skipUnless(os.name == "posix", "POSIX process groups")
    def test_timeout_kills_review_process_group_and_reaps_wrapper(self) -> None:
        process = unittest.mock.MagicMock()
        process.pid = 12345
        process.communicate.side_effect = [subprocess.TimeoutExpired("codex", 1800), (None, None)]
        process.__enter__.return_value = process
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(subprocess, "Popen", return_value=process) as launch, patch.object(os, "killpg") as kill:
                with self.assertRaises(subprocess.TimeoutExpired):
                    local_review.run_review_process(["codex"], root, {}, "review", root / "stderr.log")
            self.assertTrue(launch.call_args.kwargs["start_new_session"])
            kill.assert_called_once_with(12345, local_review.signal.SIGKILL)
            self.assertEqual(process.communicate.call_count, 2)

    def test_real_review_process_timeout_retains_stderr(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            diagnostic = root / "stderr.log"
            script = "import sys,time; print('partial review', file=sys.stderr, flush=True); time.sleep(10)"
            with patch.object(local_review, "REVIEW_TIMEOUT_SECONDS", 0.2):
                with self.assertRaises(subprocess.TimeoutExpired):
                    local_review.run_review_process([sys.executable, "-c", script], root, dict(os.environ), "review", diagnostic)
            self.assertIn("partial review", diagnostic.read_text())


    def test_same_issue_body_does_not_expire_on_comment_timestamp(self) -> None:
        payload = {"number": 89, "title": "quality", "body": "requirements", "state": "open",
                   "html_url": "https://github.com/HiroyukiFuruno/katana-render-runtime/issues/89",
                   "updated_at": "before"}
        responses = ["git@github.com:HiroyukiFuruno/katana-render-runtime.git", json.dumps(payload)]
        with patch("local_review_state.command", side_effect=responses):
            before = issue_context(Path.cwd(), [89])
        payload["updated_at"] = "after comment"
        responses[-1] = json.dumps(payload)
        with patch("local_review_state.command", side_effect=responses):
            self.assertEqual(before, issue_context(Path.cwd(), [89]))

    def test_codex_is_read_only_high_and_precedes_quality_lanes(self) -> None:
        arguments = local_review.review_command(Path.cwd(), Path("result.json"))
        self.assertIn("read-only", arguments)
        self.assertIn("gpt-6.1-sol", arguments)
        self.assertIn('model_reasoning_effort="high"', arguments)
        self.assertNotIn("--worktree", arguments)
        root = Path(__file__).resolve().parents[2]
        justfile = (root / "Justfile").read_text()
        section = justfile.split("\ncheck:\n", 1)[1].split("\n\n", 1)[0]
        self.assertLess(section.index("scripts/hooks/local_review.py"), section.index("run_parallel_checks.py"))

    def test_new_branch_issue_precedes_old_receipt_and_ambiguous_refs_reject(self) -> None:
        args = SimpleNamespace(issue=[], base="origin/master")
        with tempfile.TemporaryDirectory() as temporary:
            receipt = Path(temporary) / "receipt.json"
            receipt.write_text(json.dumps(receipt_payload(inputs(), review(inputs()))))
            with patch.dict(os.environ, {}, clear=True), patch.object(local_review, "command", return_value="fix\nRefs #120"):
                self.assertEqual(local_review.issue_numbers(Path.cwd(), args, receipt), [120])
            with patch.dict(os.environ, {}, clear=True), patch.object(local_review, "command", return_value="Refs #120\nRefs #121"):
                with self.assertRaisesRegex(ReviewError, "multiple"):
                    local_review.issue_numbers(Path.cwd(), args, receipt)

    def test_explicit_issue_and_environment_precede_inference(self) -> None:
        args = SimpleNamespace(issue=[123], base="origin/master")
        with patch.dict(os.environ, {"REVIEW_ISSUE": "124"}, clear=True):
            self.assertEqual(local_review.issue_numbers(Path.cwd(), args, Path("absent")), [123])
            args.issue = []
            self.assertEqual(local_review.issue_numbers(Path.cwd(), args, Path("absent")), [124])

    def test_original_structured_result_tamper_is_rejected(self) -> None:
        value = inputs()
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "receipt.json"
            path.write_text(json.dumps(receipt_payload(value, review(value))))
            original = review(value)
            original["summary"] = "modified original"
            path.with_suffix(".review.json").write_text(json.dumps(original))
            with self.assertRaisesRegex(ReviewError, "original structured"):
                local_review.read_receipt(path, value)


if __name__ == "__main__":
    unittest.main()
