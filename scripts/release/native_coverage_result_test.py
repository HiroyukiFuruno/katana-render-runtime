from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from typing import cast
from unittest.mock import patch

import native_coverage_result as coverage_result


def _fixture_report_path(root: Path, domain: str) -> Path:
    name = "root.json" if domain == "root" else f"{domain}.json"
    return root / coverage_result.REPORT_DIRECTORY / name


def write_fixture_report(root: Path, domain: str, count: int = 12, covered: int = 12,
                         percent: float = 100.0) -> None:
    report = {
        "type": "llvm.coverage.json.export",
        "version": "2.0.1",
        "data": [{
            "files": [{"filename": str(root / "private" / "source.rs")}],
            "totals": {"lines": {"count": count, "covered": covered, "percent": percent}},
        }],
    }
    _fixture_report_path(root, domain).write_text(json.dumps(report), encoding="utf-8")


def initialize_fixture_repository(root: Path) -> tuple[str, dict[str, str], dict[str, str]]:
    for relative in coverage_result.SOURCE_INPUTS:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(f"input:{relative}\n", encoding="utf-8")
    test_only_source = root / "scripts/html/performance-harness/src/main_tests.rs"
    test_only_source.parent.mkdir(parents=True, exist_ok=True)
    test_only_source.write_text("#![cfg(test)]\n", encoding="utf-8")
    (root / ".gitignore").write_text("tmp/\n", encoding="utf-8")
    subprocess.run(["git", "init", "-q"], cwd=root, check=True)
    subprocess.run(["git", "config", "user.name", "KRR coverage fixture"], cwd=root, check=True)
    subprocess.run(["git", "config", "user.email", "coverage-fixture@example.invalid"], cwd=root, check=True)
    subprocess.run(["git", "add", "-A"], cwd=root, check=True)
    subprocess.run(["git", "commit", "-qm", "fixture source"], cwd=root, check=True)
    tested_sha = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=root, check=True, text=True, capture_output=True,
    ).stdout.strip()
    report_root = root / coverage_result.REPORT_DIRECTORY
    report_root.mkdir(parents=True)
    for domain in ("root", "baseline", "candidate"):
        write_fixture_report(root, domain)
    profile = {
        key: f"profile-{index}"
        for index, key in enumerate(
            key for key in coverage_result.PROFILE_KEYS
            if key not in {"environmentSha256", "testThreads"}
        )
    }
    profile["environmentSha256"] = "a" * 64
    profile["testThreads"] = "2"
    tools = {key: f"tool {index}.0" for index, key in enumerate(coverage_result.TOOL_KEYS)}
    return tested_sha, profile, tools


class NativeCoverageResultTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.tested_sha, self.profile, self.tools = initialize_fixture_repository(self.root)
        self.report_root = self.root / coverage_result.REPORT_DIRECTORY

    def report_path(self, domain: str) -> Path:
        name = "root.json" if domain == "root" else f"{domain}.json"
        return self.report_root / name

    def git(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(["git", *arguments], cwd=self.root, text=True, capture_output=True, check=True)

    def write_report(self, domain: str, count: int, covered: int, percent: float) -> None:
        write_fixture_report(self.root, domain, count, covered, percent)

    def manifest(self) -> coverage_result.JsonObject:
        return coverage_result.create_coverage_result_manifest(
            self.root, self.tested_sha, "b" * 40, self.profile, self.tools,
        )

    def verify(self, manifest: object) -> None:
        coverage_result.verify_local_coverage_result_manifest(
            manifest, self.root, self.tested_sha, "b" * 40, self.profile, self.tools,
        )

    def verify_without_reports(self, manifest: object) -> None:
        coverage_result.verify_coverage_result_manifest(
            manifest, self.root, self.tested_sha, "b" * 40, self.profile, self.tools,
        )

    def test_real_reports_bind_three_domains_and_keep_privacy_allowlist(self) -> None:
        manifest = self.manifest()
        self.verify(manifest)
        self.assertEqual(manifest["testedSha"], self.tested_sha)
        self.assertEqual(manifest["workflowSha"], "b" * 40)
        domains = cast(list[dict[str, object]], manifest["domains"])
        self.assertEqual([row["domain"] for row in domains], list(coverage_result.DOMAINS))
        self.assertEqual([cast(dict[str, object], row["lines"])["unmeasured"] for row in domains], [0, 0, 0])
        serialized = json.dumps(manifest)
        self.assertNotIn(str(self.root), serialized)
        self.assertNotIn("files", serialized)
        self.assertEqual(manifest["authenticity"], "unsigned-local-result")

    def test_root_domain_uses_fixed_nextest_report_command_and_maps_threads(self) -> None:
        selected = self.root / "target" / "llvm-cov-target"
        with patch.object(coverage_result, "_coverage_selection", return_value=lambda *args, **kwargs: ("report", selected)):
            command = coverage_result._domain_command(self.root, "root", "root.json", "3")
        self.assertEqual(command["justTarget"], "root-test-and-coverage")
        self.assertEqual(command["selectedTarget"], "target/llvm-cov-target")
        self.assertEqual(command["argv"], [
            "cargo", "llvm-cov", "report", "--workspace", "--profile", "krr-coverage", "--locked", "--json",
            "--show-missing-lines", "--output-path", "tmp/quality/standalone-performance-coverage/root.json",
        ])
        self.assertEqual(command["testScope"], {
            "initialArgv": [
                "cargo", "llvm-cov", "nextest", "--workspace", "--all-targets", "--all-features", "--locked",
                "--cargo-profile", "krr-coverage", "--no-report", "--no-fail-fast", "--retries", "0",
                "--user-config-file", "scripts/release/nextest-local-user.toml", "--test-threads=3",
            ],
            "retry": "authenticated-full-run-uuid",
            "artifacts": "complete-nextest-state-and-llvm-cov-target-trees",
        })

    def test_performance_domains_bind_the_fixed_coverage_profile(self) -> None:
        selected = self.root / "target" / "llvm-cov-performance-baseline"
        with patch.object(coverage_result, "_coverage_selection", return_value=lambda *args, **kwargs: ("test", selected)):
            command = coverage_result._domain_command(
                self.root, "performance-baseline", "baseline.json", "default"
            )
        self.assertEqual(command["argv"], [
            "cargo", "llvm-cov", "--manifest-path", "scripts/html/performance-harness/Cargo.toml",
            "--workspace", "--all-targets", "--no-default-features", "--features", "baseline",
            "--locked", "--profile", "krr-coverage", "--ignore-filename-regex",
            coverage_result._coverage_test_only_ignore_regex(), "--json", "--output-path",
            "tmp/quality/standalone-performance-coverage/baseline.json",
        ])
        for mutation in ("missing", "wrong"):
            manifest = self.manifest()
            domains = cast(list[dict[str, object]], manifest["domains"])
            performance = next(domain for domain in domains if domain["domain"] == "performance-baseline")
            command = cast(dict[str, object], performance["command"])
            argv = list(cast(list[str], command["argv"]))
            if mutation == "missing":
                profile_index = argv.index("--profile")
                del argv[profile_index : profile_index + 2]
            else:
                argv[argv.index("krr-coverage")] = "wrong-profile"
            command["argv"] = argv
            with self.subTest(mutation=mutation), self.assertRaises(coverage_result.CoverageResultError):
                self.verify(manifest)

    def test_tested_workflow_identity_and_exact_domains_are_bound(self) -> None:
        for key in ("testedSha", "workflowSha"):
            manifest = self.manifest()
            manifest[key] = "c" * 40
            with self.subTest(key=key), self.assertRaisesRegex(coverage_result.CoverageResultError, "expected identity"):
                self.verify(manifest)
        malformed = self.manifest()
        domains = cast(list[dict[str, object]], malformed["domains"])
        domains[1] = dict(domains[0])
        with self.assertRaises(coverage_result.CoverageResultError):
            self.verify(malformed)
        manifest = self.manifest()
        with self.assertRaisesRegex(coverage_result.CoverageResultError, "full commit IDs"):
            coverage_result.verify_coverage_result_manifest(
                manifest, self.root, self.tested_sha, "short", self.profile, self.tools,
            )
        malformed = self.manifest()
        cast(dict[str, object], malformed["coveragePolicy"])["unmeasuredLines"] = False
        with self.assertRaisesRegex(coverage_result.CoverageResultError, "100/0 requirement"):
            self.verify_without_reports(malformed)
        malformed = self.manifest()
        domains = cast(list[dict[str, object]], malformed["domains"])
        command = cast(dict[str, object], domains[0]["command"])
        command["argv"] = ["cargo", "llvm-cov", "--fail-uncovered-lines", "1"]
        with self.assertRaises(coverage_result.CoverageResultError):
            self.verify(malformed)

    def test_allowlisted_profile_and_tool_drift_rejects_expected_context(self) -> None:
        for field in coverage_result.PROFILE_KEYS:
            manifest = self.manifest()
            drift_value = "c" * 64 if field == "environmentSha256" else f"drift-{field}"
            cast(dict[str, object], manifest["profile"])[field] = drift_value
            with self.subTest(category="profile", field=field), self.assertRaisesRegex(
                coverage_result.CoverageResultError, "independently expected",
            ):
                self.verify_without_reports(manifest)
        for field in coverage_result.TOOL_KEYS:
            manifest = self.manifest()
            cast(dict[str, object], manifest["tools"])[field] = f"drift {field}"
            with self.subTest(category="tools", field=field), self.assertRaisesRegex(
                coverage_result.CoverageResultError, "independently expected",
            ):
                self.verify_without_reports(manifest)

    def test_cache_manifest_validation_does_not_require_reports(self) -> None:
        manifest = self.manifest()
        for domain in coverage_result.DOMAINS:
            report_domain = domain.removeprefix("performance-") if domain != "root" else "root"
            self.report_path(report_domain).unlink()
        self.verify_without_reports(manifest)
        with self.assertRaisesRegex(coverage_result.CoverageResultError, "unavailable"):
            self.verify(manifest)

    def test_source_change_during_production_is_rejected(self) -> None:
        original = coverage_result._report_record
        changed = False

        def mutate_source_after_read(root: Path, domain: str, threads: str) -> coverage_result.JsonObject:
            nonlocal changed
            record = original(root, domain, threads)
            if not changed:
                (root / "Cargo.toml").write_text("changed during report read\n", encoding="utf-8")
                changed = True
            return record

        with patch.object(coverage_result, "_report_record", side_effect=mutate_source_after_read):
            with self.assertRaisesRegex(coverage_result.CoverageResultError, "clean, pinned source tree"):
                self.manifest()

    def test_uncovered_or_unmeasured_lines_reject_each_domain(self) -> None:
        for domain in coverage_result.DOMAINS:
            with self.subTest(domain=domain):
                self.write_report(domain.removeprefix("performance-") if domain != "root" else "root",
                                  12, 11, 91.67)
                with self.assertRaisesRegex(coverage_result.CoverageResultError, "100/0"):
                    self.manifest()
                self.write_report(domain.removeprefix("performance-") if domain != "root" else "root",
                                  12, 12, 100.0)

    def test_collected_line_totals_accept_partial_but_strict_totals_reject(self) -> None:
        self.write_report("root", 12, 11, 91.666666)
        report = json.loads(self.report_path("root").read_text())
        collected = coverage_result._collected_line_totals(report, "root")
        self.assertEqual(collected["count"], 12)
        self.assertEqual(collected["covered"], 11)
        self.assertEqual(collected["unmeasured"], 1)
        with self.assertRaisesRegex(coverage_result.CoverageResultError, "100/0"):
            coverage_result._line_totals(report, "root")

    def test_collected_root_lines_accept_full_export_with_function_details(self) -> None:
        report = {
            "type": "llvm.coverage.json.export",
            "version": "2.0.1",
            "data": [{
                "files": [{"filename": "source.rs", "summary": {}}],
                "functions": [{
                    "name": "f",
                    "count": 1,
                    "filenames": ["source.rs"],
                    "regions": [[1, 1, 1, 2, 1, 0, 0, 0]],
                }],
                "totals": {"lines": {"count": 12, "covered": 12, "percent": 100.0}},
            }],
        }
        self.assertEqual(coverage_result._collected_line_totals(report, "root"), {
            "count": 12, "covered": 12, "unmeasured": 0, "percent": 100.0,
        })

    def test_authenticated_collection_rows_raise_typed_policy_error(self) -> None:
        partial = {"domain": "root", "lines": {
            "count": 10, "covered": 9, "unmeasured": 1, "percent": 90.0,
        }}
        with self.assertRaises(coverage_result.CoveragePolicyError):
            coverage_result.validate_collection_policy([partial], ("root",))
        full = {"domain": "root", "lines": {
            "count": 10, "covered": 10, "unmeasured": 0, "percent": 100.0,
        }}
        coverage_result.validate_collection_policy([full], ("root",))

    def test_zero_denominator_and_inconsistent_percent_reject(self) -> None:
        self.write_report("root", 0, 0, 0.0)
        with self.assertRaisesRegex(coverage_result.CoverageResultError, "100/0"):
            self.manifest()
        self.write_report("root", 12, 12, 99.0)
        with self.assertRaisesRegex(coverage_result.CoverageResultError, "disagrees"):
            self.manifest()

    def test_stale_output_hash_missing_domain_and_unknown_fields_reject(self) -> None:
        manifest = self.manifest()
        self.report_path("candidate").write_text("{}", encoding="utf-8")
        with self.assertRaises(coverage_result.CoverageResultError):
            self.verify(manifest)
        self.write_report("candidate", 12, 12, 100.0)
        with self.assertRaises(coverage_result.CoverageResultError):
            self.report_path("baseline").unlink()
            self.manifest()
        self.write_report("baseline", 12, 12, 100.0)
        malformed = self.manifest()
        malformed["privatePath"] = str(self.root)
        with self.assertRaisesRegex(coverage_result.CoverageResultError, "privacy allowlist"):
            self.verify(malformed)

    def test_profile_paths_and_unapproved_metadata_are_rejected(self) -> None:
        profile = dict(self.profile)
        profile["imageVersion"] = str(self.root)
        with self.assertRaisesRegex(coverage_result.CoverageResultError, "privacy allowlist"):
            coverage_result.create_coverage_result_manifest(self.root, self.tested_sha, "b" * 40, profile, self.tools)
        tools = dict(self.tools)
        tools["hostPath"] = "/private/toolchain"
        with self.assertRaisesRegex(coverage_result.CoverageResultError, "privacy allowlist"):
            coverage_result.create_coverage_result_manifest(self.root, self.tested_sha, "b" * 40, self.profile, tools)
        for digest in ("A" * 64, "a" * 63, "g" * 64):
            profile = dict(self.profile)
            profile["environmentSha256"] = digest
            with self.subTest(environment_sha256=digest), self.assertRaisesRegex(
                coverage_result.CoverageResultError, "lowercase SHA-256",
            ):
                coverage_result.create_coverage_result_manifest(
                    self.root, self.tested_sha, "b" * 40, profile, self.tools,
                )


if __name__ == "__main__":
    unittest.main()
