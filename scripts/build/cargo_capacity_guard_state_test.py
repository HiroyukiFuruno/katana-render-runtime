from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from collections.abc import Callable
from pathlib import Path

import cargo_capacity_guard_state as state_schema


def _reference(value: dict[str, object]) -> dict[str, object]:
    return state_schema._object(value["reference"], "reference")


class CapacityStateSchemaTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve() / "repo"
        self.root.mkdir()
        self.target = self.root / "target"
        self.target.mkdir()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_policy(self) -> Path:
        path = self.root / state_schema.REFERENCE_POLICY
        path.parent.mkdir(parents=True, exist_ok=True)
        policy = {
            "schema": state_schema.REFERENCE_SCHEMA,
            "id": state_schema.REFERENCE_ID,
            "baseline_bytes": state_schema.REFERENCE_BASELINE_BYTES,
            "factor": state_schema.REFERENCE_FACTOR,
            "metric": state_schema.METRIC,
            "metric_version": state_schema.METRIC_VERSION,
            "baseline_observed_at": state_schema.REFERENCE_OBSERVED_AT,
            "source_state_sha256": state_schema.REFERENCE_SOURCE_STATE_SHA256,
        }
        path.write_text(json.dumps(policy), encoding="utf-8")
        return path

    def test_legacy_state_is_returned_unchanged(self) -> None:
        legacy: dict[str, object] = {"baseline_bytes": 123, "extension": [1, 2]}
        self.assertIs(state_schema.validate_state(legacy, self.root, self.target), legacy)

    def test_rejects_schema_less_records_with_v2_markers_and_non_objects(self) -> None:
        for record in (
            {"kind": "runner_measurement"},
            {"kind": "repository_reference", "reference": {}},
        ):
            with self.subTest(record=record), self.assertRaises(ValueError):
                state_schema.validate_state(record, self.root, self.target)
        with self.assertRaises(ValueError):
            state_schema.validate_state(["not", "an", "object"], self.root, self.target)

    def test_factory_uses_only_supplied_measurement_and_does_not_write_state(self) -> None:
        result = state_schema.create_runner_measurement(self.root, self.target, 987, 123.5)
        self.assertEqual(result["baseline_bytes"], 987)
        self.assertEqual(result["baseline_observed_at"], 123.5)
        self.assertEqual(result["target"], str(self.target))
        self.assertEqual(
            state_schema.validate_state(result, self.root, self.target), result
        )
        self.assertEqual(list(self.root.iterdir()), [self.target])

    def test_runner_measurement_accepts_canonical_contained_custom_target(self) -> None:
        custom_target = self.root / "build-output"
        custom_target.mkdir()
        result = state_schema.create_runner_measurement(self.root, custom_target, 321, 123.5)
        self.assertEqual(result["target"], str(custom_target))
        self.assertEqual(
            state_schema.validate_state(result, self.root, custom_target), result
        )

    def test_runner_measurement_rejects_root_and_outside_targets(self) -> None:
        outside_target = self.root.parent / "outside-target"
        outside_target.mkdir()
        for invalid_target in (self.root, outside_target):
            with self.subTest(target=invalid_target), self.assertRaises(ValueError):
                state_schema.create_runner_measurement(self.root, invalid_target, 123, 123.5)

    def test_runner_validation_allows_missing_target_but_factory_requires_it(self) -> None:
        state = state_schema.create_runner_measurement(self.root, self.target, 456, 123.5)
        self.target.rmdir()

        self.assertIs(state_schema.validate_state(state, self.root, self.target), state)
        with self.assertRaises(ValueError):
            state_schema.create_runner_measurement(self.root, self.target, 789, 124.5)

    def test_reference_factory_pins_fixture_policy_and_allows_missing_target(self) -> None:
        self.target.rmdir()
        self.write_policy()
        result = state_schema.create_repository_reference(self.root, self.target)
        self.assertEqual(result["baseline_bytes"], 2_363_047_936)
        self.assertEqual(result["factor"], 2.0)
        self.assertEqual(result["baseline_observed_at"], 1_791_358_141.1059809)
        reference = state_schema._object(result["reference"], "reference")
        self.assertEqual(reference["sha256"], hashlib.sha256(
            (self.root / state_schema.REFERENCE_POLICY).read_bytes()
        ).hexdigest())
        self.assertEqual(state_schema.validate_state(result, self.root, self.target), result)

    def test_reference_factory_rejects_custom_descendant_target(self) -> None:
        custom_target = self.root / "build-output"
        custom_target.mkdir()
        self.write_policy()

        with self.assertRaisesRegex(ValueError, "reference target must be root/target"):
            state_schema.create_repository_reference(self.root, custom_target)

    def test_runner_measurement_rejects_invalid_shapes_and_values(self) -> None:
        base = state_schema.create_runner_measurement(self.root, self.target, 100, 123.0)
        mutations: tuple[Callable[[dict[str, object]], None], ...] = (
            lambda value: value.update(extra=True),
            lambda value: value.update(schema="unknown"),
            lambda value: value.update(kind="unknown"),
            lambda value: value.update(baseline_bytes=True),
            lambda value: value.update(baseline_bytes=0),
            lambda value: value.update(factor=True),
            lambda value: value.update(factor=float("inf")),
            lambda value: value.update(factor=10**10000),
            lambda value: value.update(factor=1),
            lambda value: value.update(metric="other"),
            lambda value: value.update(metric_version=True),
            lambda value: value.update(baseline_observed_at=float("nan")),
            lambda value: value.update(baseline_observed_at=10**10000),
            lambda value: value.update(target=str(self.root / "elsewhere")),
        )
        for index, mutate in enumerate(mutations):
            with self.subTest(mutation=index):
                altered = copy.deepcopy(base)
                mutate(altered)
                with self.assertRaises(ValueError):
                    state_schema.validate_state(altered, self.root, self.target)

    def test_repository_reference_rejects_tampering_and_untrusted_policy(self) -> None:
        self.write_policy()
        base = state_schema.create_repository_reference(self.root, self.target)
        mutations: tuple[Callable[[dict[str, object]], None], ...] = (
            lambda value: value.update(extra=True),
            lambda value: value.update(kind="runner_measurement"),
            lambda value: value.update(baseline_bytes=2_363_047_937),
            lambda value: value.update(factor=3.0),
            lambda value: value.update(baseline_observed_at=1),
            lambda value: value.update(metric="other"),
            lambda value: value.update(metric_version=True),
            lambda value: value.update(target=str(self.root / "other")),
            lambda value: _reference(value).update(sha256="b" * 64),
            lambda value: _reference(value).update(id="other"),
            lambda value: _reference(value).update(extra="rejected"),
        )
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                altered = copy.deepcopy(base)
                mutate(altered)
                with self.assertRaises(ValueError):
                    state_schema.validate_state(altered, self.root, self.target)

        policy_path = self.root / state_schema.REFERENCE_POLICY
        policy_original = json.loads(policy_path.read_text(encoding="utf-8"))
        policy_mutations: tuple[Callable[[dict[str, object]], None], ...] = (
            lambda value: value.update(baseline_bytes=2_363_047_937),
            lambda value: value.update(factor=3.0),
            lambda value: value.update(baseline_observed_at=1),
            lambda value: value.update(metric="other"),
            lambda value: value.update(metric_version=True),
            lambda value: value.update(source_state_sha256="invalid"),
            lambda value: value.update(source_state_sha256="c" * 64),
            lambda value: value.update(extra="rejected"),
        )
        for index, mutate in enumerate(policy_mutations):
            with self.subTest(policy_mutation=index):
                policy = copy.deepcopy(policy_original)
                mutate(policy)
                policy_path.write_text(json.dumps(policy), encoding="utf-8")
                with self.assertRaises(ValueError):
                    state_schema.create_repository_reference(self.root, self.target)

    def test_rejects_policy_digest_change_after_reference_state_creation(self) -> None:
        policy_path = self.write_policy()
        reference_state = state_schema.create_repository_reference(self.root, self.target)
        policy = json.loads(policy_path.read_text(encoding="utf-8"))
        policy_path.write_text(json.dumps(policy, sort_keys=True, indent=2), encoding="utf-8")
        with self.assertRaises(ValueError):
            state_schema.validate_state(reference_state, self.root, self.target)

    def test_rejects_noncanonical_target_paths_and_symlinks(self) -> None:
        with self.assertRaises(ValueError):
            state_schema.create_runner_measurement(
                self.root, self.root / "sub" / ".." / "target", 100, 123.0
            )
        link = self.root / "target-link"
        link.symlink_to(self.target, target_is_directory=True)
        with self.assertRaises(ValueError):
            state_schema.create_runner_measurement(self.root, link, 100, 123.0)

    def test_rejects_actual_target_symlink_and_root_ancestor_symlink(self) -> None:
        external_target = self.root / "external-target"
        external_target.mkdir()
        self.target.rmdir()
        self.target.symlink_to(external_target, target_is_directory=True)
        with self.assertRaises(ValueError):
            state_schema.create_runner_measurement(self.root, self.target, 100, 123.0)

        self.target.unlink()
        self.target.mkdir()
        root_alias = self.root.parent / "repo-alias"
        root_alias.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            state_schema.create_runner_measurement(root_alias, root_alias / "target", 100, 123.0)

    def test_rejects_symlinked_reference_policy(self) -> None:
        path = self.write_policy()
        alternate = path.with_name("alternate.json")
        path.replace(alternate)
        path.symlink_to(alternate)
        with self.assertRaises(ValueError):
            state_schema.create_repository_reference(self.root, self.target)


if __name__ == "__main__":
    unittest.main()
