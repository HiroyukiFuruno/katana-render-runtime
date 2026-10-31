"""Tests for the fixed runtime-script producer identity."""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import runtime_script_identity as identity
import runtime_script_profiles as profiles
import runtime_script_recipe as recipe_reader
import runtime_script_result as result
import runtime_script_tool_fingerprint_inputs as pilot


PRODUCER = "runtime-unused-script-regression"
SOURCE = "a" * 64
SOURCE_INPUTS = "f" * 64
OTHER_SOURCE = "e" * 64
OTHER_INPUTS = "9" * 64
RECIPE = "b" * 64
TOOLS = "c" * 64
INPUT = "d" * 64
REAL_PILOT_FACTS = identity._pilot_facts


class RuntimeScriptIdentityTests(unittest.TestCase):
    def setUp(self) -> None:
        pilot_patch = patch.object(identity, "_pilot_facts", return_value=("8" * 64, {"scope": "pilot"}))
        pilot_patch.start()
        self.addCleanup(pilot_patch.stop)

    def test_pilot_identity_ignores_unrelated_source_digest_but_keeps_provenance(self) -> None:
        producer = "runtime-unused-tool-fingerprint"
        recipe = recipe_reader.RuntimeScriptRecipe(profiles.producer_recipe(producer), RECIPE, ())
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve()
            with (
                patch.object(identity, "_source_state", side_effect=[(SOURCE, SOURCE_INPUTS)] * 2
                             + [(OTHER_SOURCE, OTHER_INPUTS)] * 2),
                patch.object(recipe_reader, "read_runtime_script_recipe", return_value=recipe),
            ):
                first = identity.collect_identity_report(repo, producer)
                second = identity.collect_identity_report(repo, producer)
        self.assertTrue(first["cacheable"])
        self.assertEqual(first["sourceInputScope"], pilot.SEMANTIC_SCOPE)
        self.assertEqual(first["identitySha256"], second["identitySha256"])
        self.assertNotEqual(first["sourceManifestSha256"], second["sourceManifestSha256"])

    def test_pilot_missing_inputs_fail_closed(self) -> None:
        producer = "runtime-unused-tool-fingerprint"
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(identity, "_source_state", return_value=(SOURCE, SOURCE_INPUTS)),
                patch.object(identity, "_pilot_facts", side_effect=identity._CollectionFailure("tool_profile_inputs_unavailable")),
            ):
                report = identity.collect_identity_report(Path(directory).resolve(), producer)
        self.assertFalse(report["cacheable"])
        self.assertEqual(report["missReasons"], ["tool_profile_inputs_unavailable"])

    def test_pilot_facts_bind_python_context_and_reject_unadmitted_inventory(self) -> None:
        candidate = {"schema": pilot.SCHEMA, "profile": pilot.PROFILE,
                     "admission": "candidate-only", "cacheable": False,
                     "inputsCollected": True, "inventorySha256": "8" * 64,
                     "componentDigests": {"semanticEnvironment": "7" * 64},
                     "semanticEnvironmentNames": ["PATH"]}
        tool = {"schema": "krr-runtime-bundle-tool-fingerprint-v1", "tools": {}, "environment": []}
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve()
            with (
                patch.object(pilot, "collect", return_value=candidate),
                patch.object(identity, "_run_json", return_value=tool),
                patch.object(identity, "_python_context", side_effect=[{"pythonVersion": "one"}, {"pythonVersion": "two"}]),
            ):
                first = REAL_PILOT_FACTS(repo)
                second = REAL_PILOT_FACTS(repo)
            self.assertNotEqual(first[1], second[1])
            with patch.object(pilot, "collect", return_value={**candidate, "admission": "unknown"}):
                with self.assertRaises(identity._CollectionFailure):
                    REAL_PILOT_FACTS(repo)

    def test_loader_override_is_rejected_before_spawning_tool(self) -> None:
        with (
            patch.dict(os.environ, {"LD_PRELOAD": "private-fixture"}),
            patch.object(subprocess, "run") as run,
        ):
            with self.assertRaises(identity._CollectionFailure) as error:
                identity._run_json(Path("/fixture"), ["bun"], "tool_unavailable")
        self.assertEqual(error.exception.reason, "unsupported_loader_override")
        run.assert_not_called()
    def test_execution_snapshot_reads_each_fact_once_and_matches_single_identity(self) -> None:
        producer = "runtime-unused-fragment-loader"
        recipe = recipe_reader.RuntimeScriptRecipe(profiles.producer_recipe(producer), RECIPE, ())
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve()
            with (
                patch.object(
                    identity, "_source_state", return_value=(SOURCE, SOURCE_INPUTS)
                ) as source,
                patch.object(identity, "_tool_fingerprint", return_value={}) as tool,
                patch.object(recipe_reader, "read_runtime_script_recipe", return_value=recipe),
            ):
                snapshot = identity.collect_execution_snapshot(repo, producer)
            with (
                patch.object(identity, "_source_state", side_effect=[(SOURCE, SOURCE_INPUTS)] * 2),
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
            identity, "_source_state"
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
                patch.object(identity, "_source_state", return_value=(SOURCE, SOURCE_INPUTS)),
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
                patch.object(identity, "_source_state", return_value=(SOURCE, SOURCE_INPUTS)),
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
                patch.object(
                    identity, "_source_state", side_effect=[(SOURCE, SOURCE_INPUTS)] * 2
                ) as source,
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
                patch.object(
                    identity,
                    "_source_state",
                    side_effect=[(SOURCE, SOURCE_INPUTS), (OTHER_SOURCE, SOURCE_INPUTS)],
                ),
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
                patch.object(identity, "_source_state", return_value=(SOURCE, SOURCE_INPUTS)),
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
                patch.object(identity, "_source_state", side_effect=[(SOURCE, SOURCE_INPUTS)] * 2),
                patch.object(recipe_reader, "read_runtime_script_recipe", return_value=recipe),
                patch.object(identity, "_tool_fingerprint", return_value={}),
            ):
                single = identity.collect_identity_report(repo, producer)
            with (
                patch.object(identity, "_source_state", side_effect=[(SOURCE, SOURCE_INPUTS)] * 2),
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
                    patch.object(identity, "_source_state", side_effect=[(SOURCE, SOURCE_INPUTS)] * 2),
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
                patch.object(identity, "_source_state", side_effect=[(SOURCE, SOURCE_INPUTS)] * 2),
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
                "schema": "krr-runtime-script-producer-input-v2",
                "producer": PRODUCER,
                "sourceInputsSha256": SOURCE_INPUTS,
                "recipeSha256": RECIPE,
                "toolEnvironmentSha256": identity._canonical_digest({}),
            }
        )
        self.assertEqual(input_digest, expected_input)
        self.assertEqual(final_identity, result.build_identity(PRODUCER, input_digest))

    def test_ledger_only_change_keeps_cache_identity_and_reports_full_source(self) -> None:
        first = self._collect_for_source_state((SOURCE, SOURCE_INPUTS))
        changed_ledger = self._collect_for_source_state((OTHER_SOURCE, SOURCE_INPUTS))

        self.assertEqual(first["sourceInputsSha256"], SOURCE_INPUTS)
        self.assertEqual(changed_ledger["sourceInputsSha256"], SOURCE_INPUTS)
        self.assertEqual(first["sourceManifestSha256"], SOURCE)
        self.assertEqual(changed_ledger["sourceManifestSha256"], OTHER_SOURCE)
        self.assertEqual(first["inputSha256"], changed_ledger["inputSha256"])
        self.assertEqual(first["identitySha256"], changed_ledger["identitySha256"])

    def test_non_ledger_source_change_invalidates_cache_identity(self) -> None:
        first = self._collect_for_source_state((SOURCE, SOURCE_INPUTS))
        changed_source = self._collect_for_source_state((OTHER_SOURCE, OTHER_INPUTS))

        self.assertNotEqual(first["sourceInputsSha256"], changed_source["sourceInputsSha256"])
        self.assertNotEqual(first["inputSha256"], changed_source["inputSha256"])
        self.assertNotEqual(first["identitySha256"], changed_source["identitySha256"])

    def _collect_for_source_state(self, state: tuple[str, str]) -> identity.IdentityReport:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve()
            with (
                patch.dict(os.environ, {}, clear=True),
                patch.object(identity, "_source_state", side_effect=[state, state]),
                patch.object(
                    recipe_reader,
                    "read_runtime_script_recipe",
                    return_value=recipe_reader.RuntimeScriptRecipe(
                        profiles.producer_recipe(PRODUCER), RECIPE, ()
                    ),
                ),
                patch.object(identity, "_tool_fingerprint", return_value={}),
            ):
                return identity.collect_identity_report(repo, PRODUCER)

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
                patch.object(identity, "_source_state", side_effect=[(SOURCE, SOURCE_INPUTS)] * 2),
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
                patch.object(
                    identity,
                    "_source_state",
                    side_effect=[(SOURCE, SOURCE_INPUTS), (OTHER_SOURCE, SOURCE_INPUTS)],
                ),
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
        with patch.object(identity, "_source_state") as manifest:
            report = identity.collect_identity_report(Path.cwd(), "unknown")
        self.assertEqual(report["missReasons"], ["unknown_profile"])
        manifest.assert_not_called()

    def test_source_state_delegates_to_existing_canonical_helper(self) -> None:
        state = {"sourceDigest": SOURCE, "coverageInputDigest": SOURCE_INPUTS}
        with patch(
            "runtime_script_identity.coverage_context._source_state", return_value=state
        ) as load:
            self.assertEqual(identity._source_state(Path("/repo")), (SOURCE, SOURCE_INPUTS))
        load.assert_called_once_with(Path("/repo"))
