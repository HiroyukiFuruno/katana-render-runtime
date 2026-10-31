"""Tests for the scoped runtime-script CAS child environment."""

from __future__ import annotations

import json
import os
import stat
import unittest
from contextlib import contextmanager
from collections.abc import Iterator
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import NamedTuple
from unittest.mock import MagicMock, patch

from moon_cache_contract_test import MOON_TASK_CONTEXT_VARIABLES, ROOT
from runtime_script_moon_cas import (
    RUNTIME_CAS_DIAGNOSTIC_TARGET,
    RuntimeScriptCasDiagnostic,
    _runtime_cas_controlled_failed_closed,
    _runtime_cas_operation_result,
    _runtime_cas_operation_role,
    _runtime_cas_probe_acceptance,
    _runtime_cas_has_dedicated_task_environment,
    _runtime_cas_report_is_fresh,
    _runtime_cas_repair_input_mode,
    _runtime_cas_restore_entries,
    _runtime_cas_files_match_snapshot,
    _runtime_cas_snapshot_files,
    _runtime_cas_child_environment,
)


class EpochMocks(NamedTuple):
    moon: MagicMock
    hydrated: MagicMock
    validate: MagicMock
    remove: MagicMock
    log: MagicMock


def epoch_fixture() -> tuple[RuntimeScriptCasDiagnostic, dict[Path, tuple[bytes, int]]]:
    diagnostic = RuntimeScriptCasDiagnostic.__new__(RuntimeScriptCasDiagnostic)
    diagnostic.receipt = Path("/fixture/receipt.json")
    diagnostic.junit = Path("/fixture/result.xml")
    diagnostic.outputs = (diagnostic.receipt, diagnostic.junit)
    receipt = json.dumps({"identitySha256": "identity", "epoch": "new"}).encode()
    snapshot = {diagnostic.receipt: (receipt, 0o600), diagnostic.junit: (b"junit", 0o600)}
    diagnostic.repaired_files = dict(snapshot)
    diagnostic.action_hash = "old-action"
    return diagnostic, snapshot


@contextmanager
def epoch_patches(
    diagnostic: RuntimeScriptCasDiagnostic,
    snapshot: dict[Path, tuple[bytes, int]],
    actions: list[dict[str, str]],
    inventories: list[set[str]],
) -> Iterator[EpochMocks]:
    # 状態遷移の単体検証であり、実Moon/CASの動作証跡には使わない。
    with (
        patch.object(diagnostic, "_moon_run", side_effect=actions) as moon,
        patch.object(diagnostic, "_assert_hydrated") as hydrated,
        patch.object(diagnostic, "_validate_all") as validate,
        patch.object(diagnostic, "_remove_outputs") as remove,
        patch("runtime_script_moon_cas._runtime_cas_raw_directories", side_effect=inventories),
        patch("runtime_script_moon_cas._runtime_cas_snapshot_files", return_value=snapshot),
        patch("runtime_script_moon_cas._runtime_cas_files_match_snapshot", return_value=True),
        patch("runtime_script_moon_cas._runtime_cas_expected_command", return_value=["bun"]),
        patch("runtime_script_moon_cas._runtime_cas_new_producer_log", return_value={"pin": "seed"}) as log,
    ):
        yield EpochMocks(moon, hydrated, validate, remove, log)


class RuntimeScriptMoonCasEnvironmentTest(unittest.TestCase):
    def test_operation_observer_roles_are_scoped(self) -> None:
        cases = (
            ("validate-all", "metadata"),
            ("host-verify", "metadata"),
            ("identity:runtime-unused-check", "metadata"),
            ("ensure-all", "build"),
        )
        for operation, expected in cases:
            with self.subTest(operation=operation):
                self.assertEqual(_runtime_cas_operation_role(operation), expected)
        with self.assertRaises(RuntimeError):
            _runtime_cas_operation_role("unknown")

    def test_moon_target_requires_canonical_target_and_bare_task_id(self) -> None:
        valid = {
            "MOON_TARGET": RUNTIME_CAS_DIAGNOSTIC_TARGET,
            "MOON_TASK_ID": "runtime-script-cas-diagnostic",
        }
        self.assertTrue(_runtime_cas_has_dedicated_task_environment(valid))
        for value in (
            {**valid, "MOON_TASK_ID": "wrong-task"},
            {**valid, "MOON_TARGET": "other:runtime-script-cas-diagnostic"},
            {**valid, "MOON_TASK_ID": "krr:runtime-script-cas-diagnostic"},
            {"MOON_TARGET": valid["MOON_TARGET"]},
        ):
            with self.subTest(value=value):
                self.assertFalse(_runtime_cas_has_dedicated_task_environment(value))

    def test_report_freshness_rejects_stale_or_missing_run_report(self) -> None:
        previous = "a" * 64
        self.assertTrue(_runtime_cas_report_is_fresh(previous, {"runReportSha256": "b" * 64}))
        self.assertFalse(_runtime_cas_report_is_fresh(previous, {"runReportSha256": previous}))
        self.assertFalse(_runtime_cas_report_is_fresh(previous, {}))

    def test_context_target_and_child_environment_policy(self) -> None:
        cases: tuple[tuple[str, dict[str, str], bool], ...] = (
            (
                "outside-moon",
                {"PATH": "/fixture/bin", "MOON_CACHE": "disabled"},
                False,
            ),
            (
                "diagnostic-target",
                {
                    **{name: "fixture-context" for name in MOON_TASK_CONTEXT_VARIABLES},
                    "MOON_TASK_ID": "fixture-task",
                    "MOON_TARGET": RUNTIME_CAS_DIAGNOSTIC_TARGET,
                    "PATH": "/fixture/bin",
                    "MOON_CACHE": "disabled",
                },
                True,
            ),
            (
                "other-target",
                {"MOON_TASK_ID": "fixture-task", "MOON_TARGET": "krr:other"},
                True,
            ),
            ("missing-target", {"MOON_TASK_ID": "fixture-task"}, True),
        )
        for name, source, rejected in cases:
            with self.subTest(name=name):
                if rejected:
                    with self.assertRaises(RuntimeError):
                        _runtime_cas_child_environment(source)
                    continue

                child = _runtime_cas_child_environment(source)
                self.assertEqual(child.get("PATH"), source.get("PATH"))
                self.assertEqual(child["MOON_CACHE"], "read-write")
                self.assertEqual(child["MOON_CACHE_CAS_VERIFY_INTEGRITY"], "true")
                self.assertEqual(child["MOON_EXPERIMENT_CAS_OUTPUTS_CACHE"], "true")
                for variable in MOON_TASK_CONTEXT_VARIABLES:
                    self.assertNotIn(variable, child)

    def test_operation_result_rejects_unknown_and_ambiguous_payloads(self) -> None:
        with TemporaryDirectory() as directory:
            path = Path(directory) / "stdout.raw"
            cases = (
                ("unknown", "unknown", "{}\n"),
                ("producer", "identity:not-a-producer", "{}\n"),
                ("missing", "validate-all", "unittest output\n"),
                (
                    "unknown-prefix",
                    "validate-all",
                    'other-task | {"schema":"krr-runtime-script-validation-all-v1","valid":true}\n',
                ),
                (
                    "multiple",
                    "validate-all",
                    '{"schema":"krr-runtime-script-validation-all-v1","valid":true}\n'
                    '{"schema":"krr-runtime-script-validation-all-v1","valid":true}\n',
                ),
            )
            for name, operation, contents in cases:
                with self.subTest(name=name):
                    path.write_text(contents)
                    with self.assertRaises(RuntimeError):
                        _runtime_cas_operation_result(path, operation)

            path.write_text(
                f"{RUNTIME_CAS_DIAGNOSTIC_TARGET} | "
                '{"schema":"krr-runtime-script-validation-all-v1","valid":true}\n'
            )
            self.assertEqual(
                _runtime_cas_operation_result(path, "validate-all")["valid"],
                True,
            )

    def test_controlled_failed_closed_requires_every_observed_boundary(self) -> None:
        cases = (
            ("controlled", 1, True, True, True, True),
            ("zero", 0, True, True, True, False),
            ("owned-output-present", 1, False, True, True, False),
            ("different-action", 1, True, False, True, False),
            ("producer-log-added", 1, True, True, False, False),
        )
        for name, exit_code, outputs_absent, same_action, logs_unchanged, expected in cases:
            with self.subTest(name=name):
                self.assertEqual(
                    _runtime_cas_controlled_failed_closed(
                        exit_code, outputs_absent, same_action, logs_unchanged
                    ),
                    expected,
                )

    def test_probe_acceptance_separates_repair_and_corrupt_hydration(self) -> None:
        cases = (
            (
                "controlled-repair",
                {"exit": 1, "sameAction": True},
                True,
                True,
                "controlled_failed_closed",
            ),
            (
                "explicit-corrupt-hydration",
                {
                    "exit": 0,
                    "sameAction": True,
                    "classification": "corrupt_blob_hydrated",
                    "hydrationReported": True,
                    "corruptBytesMatch": True,
                },
                False,
                True,
                "corrupt_blob_hydrated",
            ),
            (
                "unknown-nonzero",
                {"exit": 1, "sameAction": False},
                True,
                True,
                None,
            ),
            (
                "corrupt-without-hydration",
                {
                    "exit": 0,
                    "sameAction": True,
                    "classification": "corrupt_blob_hydrated",
                    "hydrationReported": False,
                    "corruptBytesMatch": True,
                },
                False,
                True,
                None,
            ),
        )
        for name, result, outputs_absent, logs_unchanged, expected in cases:
            with self.subTest(name=name):
                self.assertEqual(
                    _runtime_cas_probe_acceptance(result, outputs_absent, logs_unchanged),
                    expected,
                )

    def test_repair_input_mode_preserves_explicit_corruption(self) -> None:
        cases = (
            (
                {"classification": "corrupt_blob_hydrated"},
                False,
                "preserve-corrupt-output",
            ),
            (
                {"probeAcceptance": "controlled_failed_closed"},
                True,
                "missing-output",
            ),
            ({"classification": "nonzero_unclassified"}, True, "unclassified"),
        )
        for result, outputs_absent, expected in cases:
            with self.subTest(result=result):
                self.assertEqual(
                    _runtime_cas_repair_input_mode(result, outputs_absent), expected
                )

    def test_restore_entries_attempts_all_outputs_after_one_failure(self) -> None:
        with TemporaryDirectory(dir=ROOT / "tmp/quality") as directory:
            root = Path(directory)
            good = root / "good.json"
            bad = root / "bad.json"
            target = root / "target.json"
            good.write_bytes(b"old-good")
            target.write_bytes(b"target")
            bad.symlink_to(target)
            with self.assertRaisesRegex(RuntimeError, "runtime CAS restore errors"):
                _runtime_cas_restore_entries(
                    {bad: (b"old-bad", 0o600), good: (b"new-good", 0o640)}
                )
            self.assertEqual(good.read_bytes(), b"new-good")
            self.assertEqual(stat.S_IMODE(good.stat().st_mode), 0o640)

    def test_seed_snapshot_is_distinct_from_initial_output_and_rejects_cache_mutation(
        self,
    ) -> None:
        with TemporaryDirectory(dir=ROOT / "tmp/quality") as directory:
            path = Path(directory) / "receipt.json"
            path.write_bytes(b"initial")
            initial = _runtime_cas_snapshot_files((path,))
            path.write_bytes(b"seeded")
            seeded = _runtime_cas_snapshot_files((path,))
            self.assertNotEqual(initial[path][0], seeded[path][0])
            self.assertFalse(_runtime_cas_files_match_snapshot(initial))
            self.assertTrue(_runtime_cas_files_match_snapshot(seeded))
            path.write_bytes(b"cache-mutated")
            self.assertFalse(_runtime_cas_files_match_snapshot(seeded))

    def test_restore_blob_restores_saved_mode_through_owned_fd(self) -> None:
        with TemporaryDirectory(dir=ROOT / "tmp/quality") as directory:
            path = Path(directory) / "blob"
            path.write_bytes(b"original")
            path.chmod(0o640)
            descriptor = os.open(path, os.O_RDWR)
            try:
                os.ftruncate(descriptor, 0)
                os.write(descriptor, b"corrupt")
                os.fchmod(descriptor, 0o600)
                diagnostic = RuntimeScriptCasDiagnostic.__new__(RuntimeScriptCasDiagnostic)
                diagnostic.blob_path = path
                diagnostic.blob_bytes = b"original"
                diagnostic.blob_mode = 0o640
                diagnostic.blob_fd = descriptor
                self.assertTrue(diagnostic._restore_blob())
                self.assertEqual(path.read_bytes(), b"original")
                self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o640)
            finally:
                os.close(descriptor)

    def test_repaired_epoch_seeds_new_action_and_validates_hit_and_restore(self) -> None:
        diagnostic, snapshot = epoch_fixture()
        actions = [{"actionHash": "new-action"}] * 3
        with epoch_patches(diagnostic, snapshot, actions, [{"prior"}] * 4) as mocks:
            result = diagnostic._verify_repaired_epoch_cache(Path("/fixture/raw"))
        self.assertEqual(result["actionHash"], "new-action")
        self.assertTrue(result["hitAndRestoreValidated"])
        self.assertEqual(mocks.moon.call_count, 3)
        self.assertEqual(mocks.hydrated.call_count, 3)
        self.assertEqual(mocks.validate.call_count, 3)
        mocks.remove.assert_called_once()

    def test_repaired_epoch_rejects_old_action_and_changed_epoch(self) -> None:
        diagnostic, snapshot = epoch_fixture()
        with epoch_patches(diagnostic, snapshot, [{"actionHash": "old-action"}], [set()]):
            with self.assertRaisesRegex(RuntimeError, "new Moon action"):
                diagnostic._verify_repaired_epoch_cache(Path("/fixture/raw"))
        changed = json.dumps({"identitySha256": "identity", "epoch": "old"}).encode()
        snapshot[diagnostic.receipt] = (changed, 0o600)
        with epoch_patches(diagnostic, snapshot, [{"actionHash": "new-action"}], [set()] * 2):
            with self.assertRaisesRegex(RuntimeError, "identity or epoch"):
                diagnostic._verify_repaired_epoch_cache(Path("/fixture/raw"))

    def test_repaired_epoch_records_fresh_seed_producer_log(self) -> None:
        diagnostic, snapshot = epoch_fixture()
        inventories = [{"prior"}, {"prior", "seed"}, {"prior", "seed"}, {"prior", "seed"}]
        with epoch_patches(diagnostic, snapshot, [{"actionHash": "new-action"}] * 3, inventories) as mocks:
            result = diagnostic._verify_repaired_epoch_cache(Path("/fixture/raw"))
        self.assertEqual(result["seedProducerLog"], {"pin": "seed"})
        mocks.log.assert_called_once_with(Path("/fixture/raw"), {"prior"}, ["bun"])

    def test_repaired_epoch_rejects_changed_action_or_new_hit_log(self) -> None:
        diagnostic, snapshot = epoch_fixture()
        cases = (
            ([{"actionHash": "new-action"}, {"actionHash": "different"}],
             [{"prior"}, {"prior"}], "action changed"),
            ([{"actionHash": "new-action"}, {"actionHash": "new-action"}],
             [{"prior"}, {"prior"}, {"prior", "unexpected"}], "created a producer log"),
        )
        for actions, inventories, expected in cases:
            with self.subTest(expected=expected):
                with epoch_patches(diagnostic, snapshot, actions, inventories):
                    with self.assertRaisesRegex(RuntimeError, expected):
                        diagnostic._verify_repaired_epoch_cache(Path("/fixture/raw"))


if __name__ == "__main__":
    unittest.main()
