from __future__ import annotations

import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from typing import cast
from unittest.mock import patch

import native_coverage_result as coverage_result
import native_coverage_result_cli as result_cli
import native_coverage_local_anchor as local_anchor
import native_coverage_local_context as local_context
import native_coverage_local_result as local_result
import native_coverage_result_test as fixture_helpers

CLI = Path(__file__).with_name("native_coverage_result_cli.py").resolve()


class NativeCoverageResultCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.tested_sha, self.profile, self.tools = fixture_helpers.initialize_fixture_repository(self.root)
        self.output_dir = self.root / "tmp" / "quality" / "native-coverage-cli"
        self.output_dir.mkdir(parents=True)
        self.context_path = self.output_dir / "context.json"
        self.context = self.make_context()
        self.write_context()

    def make_context(self) -> dict[str, object]:
        return {
            "schema": coverage_result.COVERAGE_CONTEXT_SCHEMA,
            "testedSha": self.tested_sha,
            "workflowSha": "b" * 40,
            "profile": dict(self.profile),
            "tools": dict(self.tools),
        }

    def write_context(self) -> None:
        self.context_path.write_text(json.dumps(self.context), encoding="utf-8")

    def invoke(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CLI), "--repo", str(self.root), *arguments],
            cwd=self.root, text=True, capture_output=True, check=False,
        )

    def cache_key(self) -> subprocess.CompletedProcess[str]:
        return self.invoke("cache-key", "--context", str(self.context_path))

    def create(self, output: Path) -> subprocess.CompletedProcess[str]:
        return self.invoke("create", "--context", str(self.context_path), "--output", str(output))

    def test_cache_key_is_deterministic_and_does_not_need_reports(self) -> None:
        for domain in coverage_result.DOMAINS:
            report_domain = domain.removeprefix("performance-") if domain != "root" else "root"
            fixture_helpers._fixture_report_path(self.root, report_domain).unlink()
        first = self.cache_key()
        second = self.cache_key()
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertEqual(second.returncode, 0, second.stderr)
        key_result = json.loads(first.stdout)
        self.assertEqual(key_result, json.loads(second.stdout))
        self.assertEqual(set(key_result), {"schema", "key"})
        self.assertEqual(key_result["schema"], coverage_result.COVERAGE_CACHE_KEY_SCHEMA)
        self.assertRegex(cast(str, key_result["key"]), r"^[0-9a-f]{64}$")
        self.context["tools"] = {**self.tools, "python": "python drift"}
        self.write_context()
        changed = self.cache_key()
        self.assertEqual(changed.returncode, 0, changed.stderr)
        self.assertNotEqual(key_result["key"], json.loads(changed.stdout)["key"])

    def test_create_requires_reports_and_never_overwrites(self) -> None:
        output = self.output_dir / "result.json"
        created = self.create(output)
        self.assertEqual(created.returncode, 0, created.stderr)
        self.assertEqual(json.loads(created.stdout), {"created": True, "schema": coverage_result.SCHEMA})
        original = output.read_bytes()
        duplicate = self.create(output)
        self.assertEqual(duplicate.returncode, 1)
        self.assertEqual(output.read_bytes(), original)
        candidate_report = fixture_helpers._fixture_report_path(self.root, "candidate")
        candidate_report.unlink()
        missing_output = self.output_dir / "missing-reports.json"
        rejected = self.create(missing_output)
        self.assertEqual(rejected.returncode, 1)
        self.assertFalse(missing_output.exists())

    def test_validate_without_reports_is_not_authentication_or_reuse(self) -> None:
        output = self.output_dir / "result.json"
        created = self.create(output)
        self.assertEqual(created.returncode, 0, created.stderr)
        for domain in coverage_result.DOMAINS:
            report_domain = domain.removeprefix("performance-") if domain != "root" else "root"
            fixture_helpers._fixture_report_path(self.root, report_domain).unlink()
        validated = self.invoke("validate", "--context", str(self.context_path), "--manifest", str(output))
        self.assertEqual(validated.returncode, 0, validated.stderr)
        result = json.loads(validated.stdout)
        self.assertEqual(result, {
            "authentication": "not-performed", "schema": coverage_result.SCHEMA, "valid": True,
        })
        self.assertNotIn("reuse", result)
        self.context["profile"] = {**self.profile, "imageVersion": "drifted-image"}
        self.write_context()
        rejected = self.invoke("validate", "--context", str(self.context_path), "--manifest", str(output))
        self.assertEqual(rejected.returncode, 1)

    def test_context_unknown_fields_and_unknown_cli_arguments_reject(self) -> None:
        self.context["unexpectedPath"] = "/private/input"
        self.write_context()
        invalid_context = self.cache_key()
        self.assertEqual(invalid_context.returncode, 1)
        self.context = self.make_context()
        self.write_context()
        unknown_arg = self.invoke("cache-key", "--context", str(self.context_path), "--extra")
        self.assertEqual(unknown_arg.returncode, 2)
        unknown_command = self.invoke("cache-all", "--context", str(self.context_path))
        self.assertEqual(unknown_command.returncode, 2)

    def test_collection_finish_and_validation_commands_are_fixed_choices(self) -> None:
        finish = result_cli._parser().parse_args(
            ["--repo", str(self.root), "local-collect-finish", "--producer", "root", "--producer-exit", "0"]
        )
        validate = result_cli._parser().parse_args(
            ["--repo", str(self.root), "local-collection-validate", "--producer", "performance"]
        )
        self.assertEqual(finish.command, "local-collect-finish")
        self.assertEqual(validate.command, "local-collection-validate")
        with self.assertRaises(SystemExit):
            result_cli._parser().parse_args(
                ["--repo", str(self.root), "local-collect-finish", "--producer", "arbitrary", "--producer-exit", "0"]
            )

    def test_local_diagnose_is_read_only_and_redacts_environment_values(self) -> None:
        existing_git_names = sorted(name for name in os.environ if name.startswith("GIT_"))
        context = {
            "schema": "krr-native-local-coverage-context-v1",
            "cacheable": False,
            "missReasons": ["unsupported_tool_override"],
            "host": {},
            "tools": {},
            "environmentSha256": "a" * 64,
            "resourcesSha256": "b" * 64,
            "hostContextSha256": "c" * 64,
        }
        anchor = {
            "schema": "krr-native-local-coverage-anchor-v1",
            "producer": "root",
            "state": "valid",
            "epoch": "d" * 64,
            "identitySha256": "e" * 64,
            "receiptSha256": "f" * 64,
        }
        with patch.object(local_result, "_local_context", return_value=context) as context_mock, \
             patch.object(local_anchor, "_git_directory", return_value=self.root / ".git"), \
             patch.object(local_anchor, "_anchor_directory", return_value=self.root / ".git") as directory_mock, \
             patch.object(local_anchor, "_read", return_value=anchor) as read_mock, \
             patch.object(local_anchor, "prepare", side_effect=AssertionError("diagnose must not prepare")), \
             patch.object(local_anchor, "begin_runtime_script", side_effect=AssertionError("diagnose must not begin")), \
             patch.object(local_anchor, "commit", side_effect=AssertionError("diagnose must not commit")), \
             patch.object(local_anchor, "invalidate", side_effect=AssertionError("diagnose must not invalidate")), \
             patch.object(local_result, "local_key", side_effect=AssertionError("diagnose must not key")), \
             patch.object(local_result, "local_begin", side_effect=AssertionError("diagnose must not begin")), \
             patch.object(local_result, "local_finish", side_effect=AssertionError("diagnose must not finish")), \
             patch.object(local_result, "local_validate", side_effect=AssertionError("diagnose must not validate")), \
             patch.dict("os.environ", {"GIT_DIR": "/private/git", "SAFE_ENV": "secret"}, clear=False):
            output = io.StringIO()
            with redirect_stdout(output):
                exit_code = result_cli.main(["--repo", str(self.root), "local-diagnose", "--producer", "root"])
        self.assertEqual(exit_code, 0)
        payload = json.loads(output.getvalue())
        self.assertEqual(payload["schema"], "krr-native-local-coverage-diagnosis-v1")
        self.assertFalse(payload["contextCacheable"])
        self.assertEqual(payload["missReasons"], ["unsupported_tool_override"])
        self.assertEqual(payload["gitEnvironmentNames"], sorted({*existing_git_names, "GIT_DIR"}))
        self.assertTrue(payload["gitDirectoryAvailable"])
        self.assertTrue(payload["anchorDirectoryAvailable"])
        self.assertTrue(payload["anchorReadable"])
        context_mock.assert_called_once_with(self.root)
        directory_mock.assert_called_once_with(self.root, create=False)
        read_mock.assert_called_once_with(self.root / ".git", "root")
        self.assertNotIn("/private/git", output.getvalue())
        self.assertNotIn("secret", output.getvalue())
        self.assertNotIn('"epoch"', output.getvalue())
        self.assertNotIn('"identitySha256"', output.getvalue())

    def test_local_diagnose_parser_requires_fixed_producer(self) -> None:
        parsed = result_cli._parser().parse_args(
            ["--repo", str(self.root), "local-diagnose", "--producer", "performance"]
        )
        self.assertEqual(parsed.command, "local-diagnose")
        with self.assertRaises(SystemExit):
            result_cli._parser().parse_args(
                ["--repo", str(self.root), "local-diagnose", "--producer", "arbitrary"]
            )

    def test_failure_payloads_distinguish_policy_from_invalid_collection(self) -> None:
        policy_output = io.StringIO()
        with patch.object(result_cli, "_run", side_effect=coverage_result.CoveragePolicyError("below 100/0")), \
             redirect_stdout(policy_output):
            policy = result_cli.main(["--repo", str(self.root), "local-validate", "--producer", "root"])
        self.assertEqual(policy, 1)
        self.assertEqual(json.loads(policy_output.getvalue()), {"failure": "coverage-policy", "valid": False})
        invalid_output = io.StringIO()
        with patch.object(result_cli, "_run", side_effect=coverage_result.CoverageResultError("receipt changed")), \
             redirect_stdout(invalid_output):
            invalid = result_cli.main(["--repo", str(self.root), "local-validate", "--producer", "root"])
        self.assertEqual(invalid, 1)
        self.assertEqual(json.loads(invalid_output.getvalue()), {"failure": "invalid-collection", "valid": False})

    def test_collect_context_is_registered_without_accepting_context_input(self) -> None:
        result = self.invoke("collect-context", "--context", str(self.context_path))
        self.assertEqual(result.returncode, 2)
        self.assertIn("unrecognized arguments", result.stderr)

    def test_font_context_dispatches_to_supported_host_inventory(self) -> None:
        expected = {
            "schema": "krr-font-environment-v1", "cacheable": True,
            "platform": "Darwin", "inventorySha256": "a" * 64,
            "recordCount": 3, "missReasons": [], "pythonVersion": "3.14.3",
        }
        output = io.StringIO()
        with patch.object(local_context, "collect_font_inventory", return_value=("a" * 64, 3)) as collect, \
             patch("native_coverage_result_cli.sys.platform", "darwin"), redirect_stdout(output):
            exit_code = result_cli.main(["--repo", str(self.root), "font-context"])
        self.assertEqual(exit_code, 0)
        actual = json.loads(output.getvalue())
        self.assertEqual(actual["schema"], expected["schema"])
        self.assertEqual(actual["inventorySha256"], expected["inventorySha256"])
        self.assertEqual(actual["recordCount"], expected["recordCount"])
        self.assertRegex(actual["pythonExecutableSha256"], r"^[0-9a-f]{64}$")
        collect.assert_called_once_with()


if __name__ == "__main__":
    unittest.main()
