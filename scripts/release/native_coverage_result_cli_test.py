from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import cast

import native_coverage_result as coverage_result
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

    def test_collect_context_is_registered_without_accepting_context_input(self) -> None:
        result = self.invoke("collect-context", "--context", str(self.context_path))
        self.assertEqual(result.returncode, 2)
        self.assertIn("unrecognized arguments", result.stderr)


if __name__ == "__main__":
    unittest.main()
