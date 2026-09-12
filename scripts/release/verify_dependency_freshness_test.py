from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

MODULE_PATH = Path(__file__).with_name("verify_dependency_freshness.py")
MODULE_SPEC = importlib.util.spec_from_file_location("verify_dependency_freshness", MODULE_PATH)
assert MODULE_SPEC is not None and MODULE_SPEC.loader is not None
freshness = importlib.util.module_from_spec(MODULE_SPEC)
sys.modules[MODULE_SPEC.name] = freshness
MODULE_SPEC.loader.exec_module(freshness)


class DependencyFreshnessTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        (self.root / "crates/renderer").mkdir(parents=True)
        (self.root / "scripts/runtime-assets").mkdir(parents=True)
        (self.root / "Cargo.toml").write_text(
            "[workspace]\nmembers = [\"crates/renderer\"]\n[workspace.dependencies]\nserde = \"1\"\n",
            encoding="utf-8",
        )
        (self.root / "crates/renderer/Cargo.toml").write_text(
            "[package]\nname = \"renderer\"\nversion = \"0.1.0\"\n[dependencies]\nserde = { workspace = true }\n",
            encoding="utf-8",
        )
        (self.root / "Cargo.lock").write_text(
            "version = 4\n[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
            encoding="utf-8",
        )
        (self.root / "package.json").write_text(json.dumps({"devDependencies": {"example": "1.0.0"}}), encoding="utf-8")
        (self.root / "bun.lock").write_text(json.dumps({"packages": {"example": ["example@1.0.0", "", {}]}}), encoding="utf-8")
        (self.root / "scripts/runtime-assets/runtime-asset-common.ts").write_text(
            'const assets = [{ kind: "mermaid", version: "1.0.0", latestUrl: "https://example.test/runtime" }];\n',
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def run_check(self, responses: dict[str, object]) -> tuple[int, str, str]:
        def fake_fetch(url: str) -> bytes:
            response = responses[url]
            if isinstance(response, Exception):
                raise response
            return json.dumps(response).encode("utf-8")

        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.object(freshness, "fetch", fake_fetch), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            result = freshness.main([str(self.root)])
        return result, stdout.getvalue(), stderr.getvalue()

    @staticmethod
    def clean_responses() -> dict[str, object]:
        return {
            "https://crates.io/api/v1/crates/serde": {"crate": {"newest_version": "1.0.0"}},
            "https://registry.npmjs.org/example/latest": {"version": "1.0.0"},
            "https://example.test/runtime": {"version": "1.0.0"},
        }

    def test_clean_direct_dependencies_and_runtime_assets_pass(self) -> None:
        result, stdout, stderr = self.run_check(self.clean_responses())
        self.assertEqual(result, 0, stderr)
        self.assertIn("passed (3 direct dependencies and pinned runtime assets)", stdout)

    def test_stale_rust_dependency_rejects_release_with_repair_command(self) -> None:
        responses = self.clean_responses()
        responses["https://crates.io/api/v1/crates/serde"] = {"crate": {"newest_version": "1.1.0"}}
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 1)
        self.assertIn("Rust serde: 1.0.0 -> 1.1.0", stderr)
        self.assertIn("just depends-update-all", stderr)

    def test_stale_javascript_dependency_rejects_release(self) -> None:
        responses = self.clean_responses()
        responses["https://registry.npmjs.org/example/latest"] = {"version": "2.0.0"}
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 1)
        self.assertIn("JavaScript example: 1.0.0 -> 2.0.0", stderr)

    def test_stale_runtime_asset_rejects_release(self) -> None:
        responses = self.clean_responses()
        responses["https://example.test/runtime"] = {"version": "1.2.0"}
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 1)
        self.assertIn("Runtime asset mermaid: 1.0.0 -> 1.2.0", stderr)

    def test_transport_failure_is_fail_closed(self) -> None:
        responses = self.clean_responses()
        responses["https://example.test/runtime"] = ValueError("request failed for https://example.test/runtime: timeout")
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 1)
        self.assertIn("failed closed", stderr)
        self.assertIn("timeout", stderr)

    def test_invalid_latest_version_is_fail_closed(self) -> None:
        responses = self.clean_responses()
        responses["https://registry.npmjs.org/example/latest"] = {"version": "newest"}
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 1)
        self.assertIn("failed closed", stderr)
        self.assertIn("unsupported semantic version", stderr)

    def test_non_object_latest_response_is_fail_closed_without_traceback(self) -> None:
        responses = self.clean_responses()
        responses["https://registry.npmjs.org/example/latest"] = ["2.0.0"]
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 1)
        self.assertIn("failed closed", stderr)
        self.assertIn("latest version response must be a JSON object", stderr)
        self.assertNotIn("Traceback", stderr)


if __name__ == "__main__":
    unittest.main()
