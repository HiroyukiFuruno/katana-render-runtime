from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

MODULE_PATH = Path(__file__).with_name("verify_dependency_freshness.py")
MODULE_SPEC = importlib.util.spec_from_file_location("verify_dependency_freshness", MODULE_PATH)
assert MODULE_SPEC is not None and MODULE_SPEC.loader is not None
freshness = importlib.util.module_from_spec(MODULE_SPEC)
sys.modules[MODULE_SPEC.name] = freshness
MODULE_SPEC.loader.exec_module(freshness)


class DependencyFreshnessTest(unittest.TestCase):
    def test_semver_prerelease_precedence(self) -> None:
        versions = ["1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.beta", "1.0.0-beta", "1.0.0-beta.2", "1.0.0-beta.11", "1.0.0-rc.1", "1.0.0"]
        for older, newer in zip(versions, versions[1:]):
            with self.subTest(older=older, newer=newer):
                self.assertLess(freshness.Version.parse(older), freshness.Version.parse(newer))
        self.assertLess(freshness.Version.parse("1.0.0-99"), freshness.Version.parse("1.0.0-A"))
        self.assertLess(freshness.Version.parse("1.0.0-alpha.2"), freshness.Version.parse("1.0.0-alpha.11"))
        self.assertLess(freshness.Version.parse("1.0.0-alpha.11"), freshness.Version.parse("1.0.0-alpha.beta"))
        self.assertLess(freshness.Version.parse("1.0.0-alpha.1"), freshness.Version.parse("1.0.0-alpha.1.1"))
        self.assertEqual(freshness.Version.parse("1.0.0+001"), freshness.Version.parse("1.0.0+other"))

    def test_invalid_semver_identifiers_are_rejected(self) -> None:
        for value in ["01.0.0", "1.0.0-01", "1.0.0-alpha..1", "1.0.0+build..1", "١.0.0"]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                freshness.Version.parse(value)

    def test_latest_parses_each_supported_dependency_ecosystem(self) -> None:
        responses = {
            "https://crates.io/api/v1/crates/serde": json.dumps(
                {"crate": {"newest_version": "1.2.0"}}
            ).encode("utf-8"),
            "https://registry.npmjs.org/@scope%2Fexample/latest": json.dumps(
                {"version": "2.3.0"}
            ).encode("utf-8"),
            "https://example.test/drawio": json.dumps({"tag_name": "v3.4.0"}).encode("utf-8"),
            "https://example.test/plantuml": b"<metadata><version>1.0.0</version><version>1.1.0</version></metadata>",
            "https://example.test/mermaid": json.dumps({"version": "4.5.0"}).encode("utf-8"),
        }

        def fake_fetch(url: str) -> bytes:
            return responses[url]

        packages = [
            freshness.Package("Rust", "serde", "1.0.0"),
            freshness.Package("JavaScript", "@scope/example", "2.0.0"),
            freshness.Package("Runtime asset", "drawio|https://example.test/drawio", "3.0.0"),
            freshness.Package("Runtime asset", "plantuml|https://example.test/plantuml", "1.0.0"),
            freshness.Package("Runtime asset", "mermaid|https://example.test/mermaid", "4.0.0"),
        ]
        with patch.object(freshness, "fetch", fake_fetch):
            self.assertEqual([freshness.latest(package) for package in packages], [
                "1.2.0", "2.3.0", "3.4.0", "1.1.0", "4.5.0"
            ])

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

    @staticmethod
    def metadata_for_direct_serde(version: str = "1.0.0") -> dict[str, object]:
        member_id = "path+file:///workspace/crates/renderer#renderer@0.1.0"
        serde_id = f"registry+https://github.com/rust-lang/crates.io-index#serde@{version}"
        return {
            "workspace_members": [member_id],
            "packages": [
                {"id": member_id, "name": "renderer", "version": "0.1.0", "source": None},
                {"id": serde_id, "name": "serde", "version": version, "source": "registry+https://github.com/rust-lang/crates.io-index"},
            ],
            "resolve": {"nodes": [{"id": member_id, "deps": [{"name": "serde", "pkg": serde_id}]}]},
        }

    def run_check(
        self,
        responses: dict[str, object],
        metadata: dict[str, object] | None = None,
        rust_fresh: bool = True,
        javascript_fresh: bool = True,
    ) -> tuple[int, str, str]:
        def fake_fetch(url: str) -> bytes:
            response = responses[url]
            if isinstance(response, Exception):
                raise response
            return json.dumps(response).encode("utf-8")

        stdout, stderr = io.StringIO(), io.StringIO()
        with (
            patch.object(freshness, "fetch", fake_fetch),
            patch.object(freshness, "cargo_metadata", return_value=metadata or self.metadata_for_direct_serde()),
            patch.object(freshness, "rust_lock_is_latest_compatible", return_value=rust_fresh),
            patch.object(freshness, "js_lock_is_latest_compatible", return_value=javascript_fresh),
            contextlib.redirect_stdout(stdout),
            contextlib.redirect_stderr(stderr),
        ):
            result = freshness.main([str(self.root)])
        return result, stdout.getvalue(), stderr.getvalue()

    @staticmethod
    def clean_responses() -> dict[str, object]:
        return {
            "https://crates.io/api/v1/crates/serde": {"crate": {"newest_version": "1.0.0"}},
            "https://registry.npmjs.org/example/latest": {"version": "1.0.0"},
            "https://example.test/runtime": {"version": "1.0.0"},
        }

    def test_clean_resolved_dependencies_and_runtime_assets_pass(self) -> None:
        result, stdout, stderr = self.run_check(self.clean_responses())
        self.assertEqual(result, 0, stderr)
        self.assertIn("passed (3 resolved dependencies and pinned runtime assets)", stdout)

    def test_stale_direct_rust_manifest_dependency_rejects_release(self) -> None:
        (self.root / "Cargo.toml").write_text(
            "[workspace]\nmembers = [\"crates/renderer\"]\n[workspace.dependencies]\nserde = \"=1.0.0\"\n",
            encoding="utf-8",
        )
        responses = self.clean_responses()
        responses["https://crates.io/api/v1/crates/serde"] = {"crate": {"newest_version": "1.1.0"}}
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 1)
        self.assertIn("Rust manifest dependency serde: 1.0.0 -> 1.1.0", stderr)
        self.assertIn("just depends-update-all", stderr)

    def test_stale_non_exact_rust_manifest_dependency_rejects_release(self) -> None:
        (self.root / "Cargo.toml").write_text(
            "[workspace]\nmembers = [\"crates/renderer\"]\n[workspace.dependencies]\nserde = \"1.0.0\"\n",
            encoding="utf-8",
        )
        responses = self.clean_responses()
        responses["https://crates.io/api/v1/crates/serde"] = {"crate": {"newest_version": "1.1.0"}}
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 1)
        self.assertIn("Rust manifest dependency serde: 1.0.0 -> 1.1.0", stderr)
        self.assertIn("just depends-update-all", stderr)

    def test_stale_rust_lock_rejects_release_with_repair_command(self) -> None:
        result, _, stderr = self.run_check(self.clean_responses(), rust_fresh=False)
        self.assertEqual(result, 1)
        self.assertIn("Rust Cargo.lock is not at the latest compatible resolution", stderr)
        self.assertIn("just depends-update-all", stderr)

    def test_cargo_update_dry_run_distinguishes_zero_singular_and_plural_updates(self) -> None:
        cases = {
            "": True,
            "Locking 0 packages to latest compatible versions": True,
            "Locking 1 package to latest compatible versions": False,
            "Locking 1 package to latest compatible version": False,
            "Locking 2 packages to latest compatible versions": False,
            "Locking 0 packages (Rust 1.95.0) to latest compatible version": True,
        }
        for cargo_output, expected in cases.items():
            with self.subTest(cargo_output=cargo_output), patch.object(
                freshness.subprocess,
                "run",
                return_value=SimpleNamespace(returncode=0, stdout=cargo_output, stderr=""),
            ):
                self.assertEqual(freshness.rust_lock_is_latest_compatible(self.root), expected)

    def test_cargo_update_dry_run_forces_plain_output_when_workflow_enables_color(self) -> None:
        completed = SimpleNamespace(
            returncode=0,
            stdout="Locking 1 package to latest compatible version\n",
            stderr="",
        )
        with patch.dict("os.environ", {"CARGO_TERM_COLOR": "always"}), patch.object(
            freshness.subprocess, "run", return_value=completed
        ) as run:
            self.assertFalse(freshness.rust_lock_is_latest_compatible(self.root))

        command = run.call_args.args[0]
        self.assertIn("--color=never", command)

    def test_rust_metadata_retains_direct_and_transitive_resolution(self) -> None:
        metadata = self.metadata_for_direct_serde()
        metadata["resolve"]["nodes"][0]["deps"][0]["name"] = "renamed-serde"
        metadata["packages"].append(
            {
                "id": "registry+https://github.com/rust-lang/crates.io-index#serde@2.0.0",
                "name": "serde",
                "version": "2.0.0",
                "source": "registry+https://github.com/rust-lang/crates.io-index",
            }
        )
        result, _, stderr = self.run_check(self.clean_responses(), metadata)
        self.assertEqual(result, 0, stderr)

    def test_transitive_rust_metadata_is_accepted_when_cargo_resolution_is_fresh(self) -> None:
        metadata = self.metadata_for_direct_serde()
        transitive_id = "registry+https://github.com/rust-lang/crates.io-index#itoa@1.0.0"
        metadata["packages"].append(
            {"id": transitive_id, "name": "itoa", "version": "1.0.0", "source": "registry+https://github.com/rust-lang/crates.io-index"}
        )
        serde_id = metadata["resolve"]["nodes"][0]["deps"][0]["pkg"]
        metadata["resolve"]["nodes"].extend(
            [{"id": serde_id, "deps": [{"name": "itoa", "pkg": transitive_id}]}, {"id": transitive_id, "deps": []}]
        )
        result, _, stderr = self.run_check(self.clean_responses(), metadata)
        self.assertEqual(result, 0, stderr)

    def test_direct_rust_dependency_uses_resolved_package_id_not_dependency_name(self) -> None:
        metadata = self.metadata_for_direct_serde()
        dependency = metadata["resolve"]["nodes"][0]["deps"][0]
        dependency["name"] = "serde_legacy"
        metadata["packages"].append(
            {
                "id": "registry+https://github.com/rust-lang/crates.io-index#serde@2.0.0",
                "name": "serde",
                "version": "2.0.0",
                "source": "registry+https://github.com/rust-lang/crates.io-index",
            }
        )
        result, _, stderr = self.run_check(self.clean_responses(), metadata)
        self.assertEqual(result, 0, stderr)

    def test_missing_direct_package_metadata_fails_closed(self) -> None:
        metadata = self.metadata_for_direct_serde()
        metadata["resolve"]["nodes"][0]["deps"][0]["pkg"] = "missing-package-id"
        result, _, stderr = self.run_check(self.clean_responses(), metadata)
        self.assertEqual(result, 1)
        self.assertIn("direct Rust package missing from cargo metadata", stderr)

    def test_stale_javascript_lock_rejects_release(self) -> None:
        result, _, stderr = self.run_check(self.clean_responses(), javascript_fresh=False)
        self.assertEqual(result, 1)
        self.assertIn("JavaScript bun.lock is not at the latest compatible resolution", stderr)

    def test_transitive_javascript_dependency_is_accepted_when_bun_resolution_is_fresh(self) -> None:
        (self.root / "bun.lock").write_text(
            json.dumps({"packages": {"example": ["example@1.0.0", "", {}], "transitive": ["transitive@1.0.0", "", {}]}}),
            encoding="utf-8",
        )
        result, _, stderr = self.run_check(self.clean_responses())
        self.assertEqual(result, 0, stderr)

    def test_bun_lock_dependency_path_uses_the_resolved_package_name(self) -> None:
        (self.root / "bun.lock").write_text(
            json.dumps({"packages": {"parent/commander": ["commander@1.0.0", "", {}]}}),
            encoding="utf-8",
        )
        responses = self.clean_responses()
        responses["https://registry.npmjs.org/commander/latest"] = {"version": "1.0.0"}
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 0, stderr)

    def test_stale_javascript_prerelease_lock_rejects_release(self) -> None:
        result, _, stderr = self.run_check(self.clean_responses(), javascript_fresh=False)
        self.assertEqual(result, 1)
        self.assertIn("JavaScript bun.lock is not at the latest compatible resolution", stderr)

    def test_stale_runtime_asset_rejects_release(self) -> None:
        responses = self.clean_responses()
        responses["https://example.test/runtime"] = {"version": "1.2.0"}
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 1)
        self.assertIn("Runtime asset mermaid: 1.0.0 -> 1.2.0", stderr)

    def test_runtime_asset_catalog_properties_are_order_independent(self) -> None:
        (self.root / "scripts/runtime-assets/runtime-asset-common.ts").write_text(
            'const assets = [\n  {\n    latestUrl: "https://example.test/runtime",\n    version: "1.0.0",\n    kind: "mermaid",\n  },\n];\n',
            encoding="utf-8",
        )
        result, _, stderr = self.run_check(self.clean_responses())
        self.assertEqual(result, 0, stderr)

    def test_transport_failure_is_fail_closed(self) -> None:
        responses = self.clean_responses()
        responses["https://example.test/runtime"] = ValueError("request failed for https://example.test/runtime: timeout")
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 1)
        self.assertIn("failed closed", stderr)
        self.assertIn("timeout", stderr)

    def test_invalid_latest_version_is_fail_closed(self) -> None:
        responses = self.clean_responses()
        responses["https://example.test/runtime"] = {"version": "newest"}
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 1)
        self.assertIn("failed closed", stderr)
        self.assertIn("unsupported semantic version", stderr)

    def test_non_object_latest_response_is_fail_closed_without_traceback(self) -> None:
        responses = self.clean_responses()
        responses["https://example.test/runtime"] = ["2.0.0"]
        result, _, stderr = self.run_check(responses)
        self.assertEqual(result, 1)
        self.assertIn("failed closed", stderr)
        self.assertIn("latest version response must be a JSON object", stderr)
        self.assertNotIn("Traceback", stderr)


if __name__ == "__main__":
    unittest.main()
