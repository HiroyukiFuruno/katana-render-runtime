"""Tests for the fixed runtime-script producer identity."""

from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import runtime_script_identity as identity
import runtime_script_profiles as profiles
import runtime_script_recipe as recipe_reader
import runtime_script_result as result


PRODUCER = "runtime-unused-script-regression"
SOURCE = "a" * 64
RECIPE = "b" * 64
TOOLS = "c" * 64
INPUT = "d" * 64


class RuntimeScriptIdentityTests(unittest.TestCase):
    def test_execution_snapshot_reads_each_fact_once_and_matches_single_identity(self) -> None:
        producer = "runtime-unused-fragment-loader"
        recipe = recipe_reader.RuntimeScriptRecipe(profiles.producer_recipe(producer), RECIPE, ())
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve()
            with (
                patch.object(identity, "_source_digest", return_value=SOURCE) as source,
                patch.object(identity, "_tool_fingerprint", return_value={}) as tool,
                patch.object(recipe_reader, "read_runtime_script_recipe", return_value=recipe),
            ):
                snapshot = identity.collect_execution_snapshot(repo, producer)
            with (
                patch.object(identity, "_source_digest", side_effect=[SOURCE, SOURCE]),
                patch.object(identity, "_tool_fingerprint", return_value={}),
                patch.object(recipe_reader, "read_runtime_script_recipe", return_value=recipe),
            ):
                stable = identity.collect_identity_report(repo, producer)
        self.assertEqual(snapshot, stable)
        self.assertTrue(snapshot["cacheable"])
        self.assertEqual(source.call_count, 1)
        self.assertEqual(tool.call_count, 1)

    def test_execution_snapshot_rejects_unknown_profile_before_reading_facts(self) -> None:
        with tempfile.TemporaryDirectory() as directory, patch.object(
            identity, "_source_digest"
        ) as source:
            report = identity.collect_execution_snapshot(
                Path(directory).resolve(), "unknown-runtime-producer"
            )
        self.assertFalse(report["cacheable"])
        self.assertEqual(report["missReasons"], ["unknown_profile"])
        source.assert_not_called()

    def test_execution_snapshot_fails_closed_when_tool_context_is_unavailable(self) -> None:
        producer = "runtime-unused-fragment-loader"
        recipe = recipe_reader.RuntimeScriptRecipe(profiles.producer_recipe(producer), RECIPE, ())
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(identity, "_source_digest", return_value=SOURCE),
                patch.object(
                    identity,
                    "_tool_fingerprint",
                    side_effect=identity._CollectionFailure("tool_fingerprint_unavailable"),
                ),
                patch.object(recipe_reader, "read_runtime_script_recipe", return_value=recipe),
            ):
                report = identity.collect_execution_snapshot(Path(directory).resolve(), producer)
        self.assertFalse(report["cacheable"])
        self.assertEqual(report["missReasons"], ["tool_fingerprint_unavailable"])

    def test_execution_snapshot_rejects_cross_profile_recipe(self) -> None:
        producer = "runtime-unused-fragment-loader"
        recipe = recipe_reader.RuntimeScriptRecipe(
            profiles.producer_recipe("runtime-unused-check"), RECIPE, ()
        )
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(identity, "_source_digest", return_value=SOURCE),
                patch.object(identity, "_tool_fingerprint", return_value={}),
                patch.object(recipe_reader, "read_runtime_script_recipe", return_value=recipe),
            ):
                report = identity.collect_execution_snapshot(Path(directory).resolve(), producer)
        self.assertFalse(report["cacheable"])
        self.assertEqual(report["missReasons"], ["recipe_profile_mismatch"])

    def test_fixed_leaf_collection_shares_source_and_tool_stability_reads(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            recipes = {
                producer: recipe_reader.RuntimeScriptRecipe(
                    profiles.producer_recipe(producer), RECIPE, ()
                )
                for producer in profiles.RUNTIME_SCRIPT_PROFILES
            }
            with (
                patch.object(identity, "_source_digest", side_effect=[SOURCE, SOURCE]) as source,
                patch.object(identity, "_tool_fingerprint", return_value={}) as tool,
                patch.object(
                    recipe_reader,
                    "read_runtime_script_recipe",
                    side_effect=lambda _repo, producer: recipes[producer],
                ),
            ):
                reports = identity.collect_fixed_leaf_identity_reports(
                    Path(directory).resolve()
                )
        self.assertEqual(set(reports), set(profiles.RUNTIME_SCRIPT_PROFILES))
        self.assertTrue(all(report["cacheable"] for report in reports.values()))
        self.assertEqual(source.call_count, 2)
        self.assertEqual(tool.call_count, 2)

    def test_fixed_leaf_collection_fails_closed_for_stale_source_and_cross_profile_recipe(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(identity, "_source_digest", side_effect=[SOURCE, "f" * 64]),
                patch.object(identity, "_tool_fingerprint", return_value={}),
                patch.object(
                    recipe_reader,
                    "read_runtime_script_recipe",
                    side_effect=lambda _repo, producer: recipe_reader.RuntimeScriptRecipe(
                        profiles.producer_recipe(producer), RECIPE, ()
                    ),
                ),
            ):
                reports = identity.collect_fixed_leaf_identity_reports(
                    Path(directory).resolve()
                )
        self.assertEqual(set(reports), set(profiles.RUNTIME_SCRIPT_PROFILES))
        self.assertEqual(
            {report["missReasons"][0] for report in reports.values()},
            {"source_changed_during_collection"},
        )

    def test_fixed_leaf_collection_rejects_cross_profile_recipe_before_after_reads(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(identity, "_source_digest", return_value=SOURCE),
                patch.object(identity, "_tool_fingerprint", return_value={}),
                patch.object(
                    recipe_reader,
                    "read_runtime_script_recipe",
                    return_value=recipe_reader.RuntimeScriptRecipe(
                        profiles.producer_recipe("runtime-unused-fragment-loader"), RECIPE, ()
                    ),
                ),
            ):
                reports = identity.collect_fixed_leaf_identity_reports(
                    Path(directory).resolve()
                )
        self.assertEqual(
            {report["missReasons"][0] for report in reports.values()},
            {"recipe_profile_mismatch"},
        )

    def test_fixed_leaf_report_matches_single_profile_collection(self) -> None:
        producer = "runtime-unused-fragment-loader"
        recipe = recipe_reader.RuntimeScriptRecipe(profiles.producer_recipe(producer), RECIPE, ())
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve()
            recipe = recipe_reader.RuntimeScriptRecipe(
                profiles.producer_recipe(producer), RECIPE, ()
            )
            with (
                patch.object(identity, "_source_digest", side_effect=[SOURCE, SOURCE]),
                patch.object(recipe_reader, "read_runtime_script_recipe", return_value=recipe),
                patch.object(identity, "_tool_fingerprint", return_value={}),
            ):
                single = identity.collect_identity_report(repo, producer)
            with (
                patch.object(identity, "_source_digest", side_effect=[SOURCE, SOURCE]),
                patch.object(
                    recipe_reader,
                    "read_runtime_script_recipe",
                    side_effect=lambda _repo, leaf: recipe_reader.RuntimeScriptRecipe(
                        profiles.producer_recipe(leaf), RECIPE, ()
                    ),
                ),
                patch.object(identity, "_tool_fingerprint", return_value={}),
            ):
                fixed = identity.collect_fixed_leaf_identity_reports(repo)[producer]
        self.assertEqual(fixed, single)

    def test_rust_host_fingerprint_requires_private_digest_only_schema(self) -> None:
        cases = (
            (None, "rust_facts_host_shape_unsupported"),
            ({"schema": "unknown", "fingerprint": INPUT}, "rust_facts_host_shape_unsupported"),
            (
                {"schema": "krr-rust-facts-host-fingerprint-v1", "fingerprint": "secret"},
                "rust_facts_host_shape_unsupported",
            ),
            (
                {
                    "schema": "krr-rust-facts-host-fingerprint-v1",
                    "fingerprint": INPUT,
                    "metadata": "must not leak",
                },
                "rust_facts_host_shape_unsupported",
            ),
        )
        for payload, reason in cases:
            with self.subTest(payload=payload), patch.object(identity, "_run_json", return_value=payload):
                with self.assertRaises(identity._CollectionFailure) as raised:
                    identity._rust_host_fingerprint(Path("/repo"))
            self.assertEqual(raised.exception.reason, reason)

    def test_mathjax_evidence_fingerprint_requires_private_digest_only_schema(self) -> None:
        cases = (
            None,
            {"schema": "unknown", "fingerprint": INPUT},
            {"schema": "krr-mathjax-evidence-fingerprint-v1", "fingerprint": "secret"},
            {
                "schema": "krr-mathjax-evidence-fingerprint-v1",
                "fingerprint": INPUT,
                "metadata": "must not leak",
            },
        )
        for payload in cases:
            with self.subTest(payload=payload), patch.object(identity, "_run_json", return_value=payload):
                with self.assertRaises(identity._CollectionFailure) as raised:
                    identity._mathjax_evidence_fingerprint(Path("/repo"))
            self.assertEqual(raised.exception.reason, "mathjax_evidence_shape_unsupported")

    def test_rust_host_fingerprint_is_added_as_a_digest_only(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve()
            prepared = repo / identity.PREPARED_RUNTIME
            prepared.parent.mkdir(parents=True)
            prepared.write_bytes(b"prepared")
            with (
                patch.object(
                    identity,
                    "_run_json",
                    side_effect=[
                        {"schema": "krr-runtime-bundle-tool-fingerprint-v1", "tools": {}, "environment": []},
                        {
                            "schema": "krr-rust-facts-host-fingerprint-v1",
                            "fingerprint": INPUT,
                        },
                        {
                            "schema": "krr-mathjax-evidence-fingerprint-v1",
                            "fingerprint": INPUT,
                        },
                    ],
                ),
                patch.object(identity, "_digest_bytes", return_value=TOOLS),
            ):
                fingerprint = identity._tool_fingerprint(repo)
        self.assertEqual(fingerprint["rustFactsHostFingerprint"], INPUT)
        self.assertNotIn("metadata", fingerprint)

    def test_python_environment_digest_changes_without_exposing_environment_values(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve()
            prepared = repo / identity.PREPARED_RUNTIME
            prepared.parent.mkdir(parents=True)
            prepared.write_bytes(b"prepared")
            tool_payloads = [
                {"schema": "krr-runtime-bundle-tool-fingerprint-v1", "tools": {}, "environment": []},
                {"schema": "krr-rust-facts-host-fingerprint-v1", "fingerprint": INPUT},
                {"schema": "krr-mathjax-evidence-fingerprint-v1", "fingerprint": INPUT},
            ]
            with patch.dict(os.environ, {"PYTHONPATH": "PYTHONPATH_SENTINEL_ONE"}):
                with patch.object(identity, "_run_json", side_effect=tool_payloads):
                    first = identity._tool_fingerprint(repo)
            with patch.dict(os.environ, {"PYTHONPATH": "PYTHONPATH_SENTINEL_TWO"}):
                with patch.object(identity, "_run_json", side_effect=tool_payloads):
                    second = identity._tool_fingerprint(repo)
        self.assertNotEqual(
            first["pythonEnvironmentSha256"], second["pythonEnvironmentSha256"]
        )
        for name in ("pythonEnvironmentSha256", "pythonFlagsSha256", "pythonSearchPathSha256"):
            value = first[name]
            self.assertIsInstance(value, str)
            assert isinstance(value, str)
            self.assertRegex(value, r"^[0-9a-f]{64}$")
        serialized = repr(first)
        self.assertNotIn("PYTHONPATH_SENTINEL_ONE", serialized)
        self.assertNotIn("PYTHONPATH_SENTINEL_TWO", serialized)

    def test_verified_tool_context_change_invalidates_collection(self) -> None:
        for field in ("mathjaxEvidenceFingerprint", "rustFactsHostFingerprint"):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as directory:
                with (
                    patch.object(identity, "_source_digest", side_effect=[SOURCE, SOURCE]),
                    patch.object(
                        recipe_reader,
                        "read_runtime_script_recipe",
                        return_value=recipe_reader.RuntimeScriptRecipe(
                            profiles.producer_recipe(PRODUCER), RECIPE, ()
                        ),
                    ),
                    patch.object(
                        identity,
                        "_tool_fingerprint",
                        side_effect=[{field: INPUT}, {field: "e" * 64}],
                    ),
                ):
                    report = identity.collect_identity_report(Path(directory).resolve(), PRODUCER)
                self.assertFalse(report["cacheable"])
                self.assertEqual(report["missReasons"], ["source_changed_during_collection"])
                self.assertIsNone(report["identitySha256"])

    def test_identity_uses_single_input_digest_with_fixed_result_adapter(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve()
            with (
                patch.dict(os.environ, {}, clear=True),
                patch.object(identity, "_source_digest", side_effect=[SOURCE, SOURCE]),
                patch.object(
                    recipe_reader,
                    "read_runtime_script_recipe",
                    return_value=recipe_reader.RuntimeScriptRecipe(profiles.producer_recipe(PRODUCER), RECIPE, ()),
                ),
                patch.object(identity, "_tool_fingerprint", return_value={}),
            ):
                report = identity.collect_identity_report(repo, PRODUCER)
        self.assertTrue(report["cacheable"])
        self.assertEqual(report["recipeSha256"], RECIPE)
        input_digest = report["inputSha256"]
        final_identity = report["identitySha256"]
        self.assertIsNotNone(input_digest)
        self.assertIsNotNone(final_identity)
        assert input_digest is not None
        assert final_identity is not None
        expected_input = identity._canonical_digest(
            {
                "schema": "krr-runtime-script-producer-input-v1",
                "producer": PRODUCER,
                "sourceManifestSha256": SOURCE,
                "recipeSha256": RECIPE,
                "toolEnvironmentSha256": identity._canonical_digest({}),
            }
        )
        self.assertEqual(input_digest, expected_input)
        self.assertEqual(final_identity, result.build_identity(PRODUCER, input_digest))

    def test_moon_task_metadata_does_not_change_semantic_identity(self) -> None:
        reports = [self._collect_for_task(task_id) for task_id in ("krr:leaf", "krr:aggregate")]
        self.assertTrue(all(report["cacheable"] for report in reports))
        self.assertEqual(reports[0]["identitySha256"], reports[1]["identitySha256"])
        self.assertNotEqual(reports[0]["moonTaskHash"], reports[1]["moonTaskHash"])

    def _collect_for_task(self, task_id: str) -> identity.IdentityReport:
        moon_hash = SOURCE if task_id == "krr:leaf" else RECIPE
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.dict(
                    os.environ,
                    {"MOON_TASK_ID": task_id, "MOON_TASK_HASH": moon_hash},
                    clear=True,
                ),
                patch.object(identity, "_source_digest", side_effect=[SOURCE, SOURCE]),
                patch.object(
                    recipe_reader,
                    "read_runtime_script_recipe",
                    return_value=recipe_reader.RuntimeScriptRecipe(profiles.producer_recipe(PRODUCER), RECIPE, ()),
                ),
                patch.object(identity, "_tool_fingerprint", return_value={}),
            ):
                return identity.collect_identity_report(Path(directory).resolve(), PRODUCER)

    def test_source_change_during_collection_is_a_miss(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(identity, "_source_digest", side_effect=[SOURCE, "f" * 64]),
                patch.object(
                    recipe_reader,
                    "read_runtime_script_recipe",
                    return_value=recipe_reader.RuntimeScriptRecipe(profiles.producer_recipe(PRODUCER), RECIPE, ()),
                ),
                patch.object(identity, "_tool_fingerprint", return_value={}),
            ):
                report = identity.collect_identity_report(Path(directory).resolve(), PRODUCER)
        self.assertFalse(report["cacheable"])
        self.assertEqual(report["missReasons"], ["source_changed_during_collection"])
        self.assertIsNone(report["identitySha256"])

    def test_unknown_producer_is_a_miss_without_running_collectors(self) -> None:
        with patch.object(identity, "_source_digest") as manifest:
            report = identity.collect_identity_report(Path.cwd(), "unknown")
        self.assertEqual(report["missReasons"], ["unknown_profile"])
        manifest.assert_not_called()

    def test_source_digest_delegates_to_existing_canonical_helper(self) -> None:
        with patch("runtime_script_identity.coverage_context._source_digest", return_value=SOURCE) as load:
            self.assertEqual(identity._source_digest(Path("/repo")), SOURCE)
        load.assert_called_once_with(Path("/repo"))
