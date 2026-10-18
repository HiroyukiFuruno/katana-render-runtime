from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import native_coverage_local_anchor as local_anchor
import native_coverage_result_test as fixture_helpers
import runtime_script_cache_cli as cache_cli
import runtime_script_identity as identity_collector
import runtime_script_producer as producer_runner
import runtime_script_profiles as profiles
import runtime_script_recipe as recipe_reader
import runtime_script_result as result
import runtime_script_result_verify as result_verify

PRODUCER = cache_cli.PRODUCER
IDENTITY = "a" * 64
SOURCE_MANIFEST = "b" * 64
RECIPE = "c" * 64
TOOLS = "d" * 64


def identity_report(
    identity: str = IDENTITY,
    *,
    producer: str = PRODUCER,
    cacheable: bool = True,
) -> dict[str, object]:
    return {
        "schema": "krr-runtime-script-identity-v1",
        "producer": producer,
        "cacheable": cacheable,
        "missReasons": [] if cacheable else ["collector_unavailable"],
        "sourceManifestSha256": SOURCE_MANIFEST,
        "recipeSha256": RECIPE,
        "toolEnvironmentSha256": TOOLS,
        "inputSha256": "e" * 64,
        "identitySha256": identity if cacheable else None,
        "moonTaskHash": None,
    }


class RuntimeScriptCacheCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        fixture_helpers.initialize_fixture_repository(self.root)

    def git_directory(self) -> Path:
        result = subprocess.run(
            ["git", "rev-parse", "--absolute-git-dir"],
            cwd=self.root,
            check=True,
            text=True,
            capture_output=True,
        )
        return Path(result.stdout.strip())

    def recipe(self) -> recipe_reader.RuntimeScriptRecipe:
        return recipe_reader.RuntimeScriptRecipe(
            recipe_reader.RECIPE_NAME,
            RECIPE,
            (("bun", "test", "--path-ignore-patterns", "tmp/**", "./scripts/runtime-bundles/fixed_test.ts"),),
        )

    def fake_run(
        self, identity: str, receipt_bytes: bytes = b"fresh-success"
    ) -> object:
        def run(
            repo: Path,
            producer: str,
            input_sha256: str,
            recipe: recipe_reader.RuntimeScriptRecipe,
        ) -> producer_runner.RuntimeScriptRun:
            del input_sha256, recipe
            epoch = local_anchor.prepare(repo, producer, identity)
            if epoch is None or not local_anchor.begin_runtime_script(repo, producer, identity, epoch):
                raise AssertionError("fake producer could not prepare anchor")
            self.assertTrue(local_anchor.commit(repo, producer, identity, epoch, receipt_bytes))
            return producer_runner.RuntimeScriptRun(
                producer,
                identity,
                epoch,
                0,
                "tmp/quality/runtime-script-results/raw/run-test",
                f"tmp/quality/runtime-script-results/{producer}.json",
                (),
            )

        return run

    def test_same_source_current_key_is_stable_and_binds_source_pins(self) -> None:
        with patch.object(
            identity_collector,
            "collect_identity_report",
            return_value=identity_report(),
        ) as collect:
            first = cache_cli.current_key(self.root)
            second = cache_cli.current_key(self.root)

        self.assertEqual(first, second)
        self.assertEqual(first["schema"], "krr-runtime-script-key-v1")
        self.assertEqual(first["producer"], PRODUCER)
        self.assertEqual(collect.call_count, 2)
        report = collect.call_args.args
        self.assertEqual(report[0], self.root)
        self.assertEqual(report[1], PRODUCER)
        anchor_dir = self.git_directory() / local_anchor.DIRECTORY_NAME
        anchor_path = anchor_dir / local_anchor.PRODUCERS[PRODUCER]
        self.assertTrue(anchor_path.is_file())
        self.assertEqual(anchor_path.stat().st_mode & 0o777, 0o600)

    def test_validation_failure_changes_key_and_rejects_old_success(self) -> None:
        with patch.object(
            identity_collector,
            "collect_identity_report",
            return_value=identity_report(),
        ):
            first_key = cache_cli.current_key(self.root)
            anchor_dir = self.git_directory() / local_anchor.DIRECTORY_NAME
            anchor_path = anchor_dir / local_anchor.PRODUCERS[PRODUCER]
            old_anchor = local_anchor._read(anchor_dir, PRODUCER)
            self.assertIsNotNone(old_anchor)
            old_epoch = str(old_anchor["epoch"] if old_anchor else "")
            old_success = b"old-success-receipt"
            self.assertTrue(local_anchor.commit(self.root, PRODUCER, IDENTITY, old_epoch, old_success))
            self.assertTrue(local_anchor.verify(self.root, PRODUCER, IDENTITY, old_epoch, old_success))

            with patch.object(
                result_verify,
                "verify_receipt",
                side_effect=result.RuntimeScriptResultError("stale receipt"),
            ), self.assertRaises(result.RuntimeScriptResultError):
                cache_cli.validate_current(self.root)

            second_key = cache_cli.current_key(self.root)

        self.assertNotEqual(first_key["fingerprint"], second_key["fingerprint"])
        self.assertFalse(local_anchor.verify(self.root, PRODUCER, IDENTITY, old_epoch, old_success))
        self.assertTrue(anchor_path.is_file())

    def test_validation_success_keeps_key_and_anchor_epoch(self) -> None:
        with patch.object(
            identity_collector,
            "collect_identity_report",
            return_value=identity_report(),
        ):
            first_key = cache_cli.current_key(self.root)
            anchor_dir = self.git_directory() / local_anchor.DIRECTORY_NAME
            before = local_anchor._read(anchor_dir, PRODUCER)
            self.assertIsNotNone(before)
            with patch.object(result_verify, "verify_receipt", return_value={"producer": PRODUCER}):
                self.assertTrue(cache_cli.validate_current(self.root)["valid"])
            after = local_anchor._read(anchor_dir, PRODUCER)
            second_key = cache_cli.current_key(self.root)

        self.assertEqual(first_key, second_key)
        self.assertEqual(before, after)

    def test_validate_all_checks_each_fixed_profile(self) -> None:
        reports = {
            producer: identity_report(
                f"{index + 1:064x}", producer=producer
            )
            for index, producer in enumerate(profiles.RUNTIME_SCRIPT_PROFILES)
        }
        with (
            patch.object(
                identity_collector,
                "collect_fixed_leaf_identity_reports",
                return_value=reports,
            ) as collect,
            patch.object(result_verify, "verify_receipt", return_value={"producer": PRODUCER}) as verify,
        ):
            result_report = cache_cli.validate_all(self.root)

        self.assertTrue(result_report["valid"])
        validated = result_report["producers"]
        assert isinstance(validated, list)
        self.assertEqual(len(validated), len(profiles.RUNTIME_SCRIPT_PROFILES))
        self.assertEqual(collect.call_count, 1)
        self.assertEqual(verify.call_count, len(profiles.RUNTIME_SCRIPT_PROFILES))

    def test_validate_all_rejects_missing_or_mismatched_reports(self) -> None:
        first = next(iter(profiles.RUNTIME_SCRIPT_PROFILES))
        cases = [
            {},
            {first: identity_report(producer="wrong")},
            {first: identity_report(producer=first, cacheable=False)},
        ]
        for reports in cases:
            with self.subTest(reports=reports), patch.object(
                identity_collector,
                "collect_fixed_leaf_identity_reports",
                return_value=reports,
            ), self.assertRaises(result.RuntimeScriptResultError):
                cache_cli.validate_all(self.root)

    def test_ensure_all_uses_one_batch_when_every_leaf_is_valid(self) -> None:
        reports = {
            producer: identity_report(f"{index + 1:064x}", producer=producer)
            for index, producer in enumerate(profiles.RUNTIME_SCRIPT_PROFILES)
        }
        with (
            patch.object(
                identity_collector,
                "collect_fixed_leaf_identity_reports",
                return_value=reports,
            ) as collect,
            patch.object(result_verify, "verify_receipt", return_value={"producer": PRODUCER}),
        ):
            result_report = cache_cli.ensure_all(self.root)
        self.assertFalse(result_report["recovered"])
        self.assertEqual(collect.call_count, 1)

    def test_ensure_all_recovers_only_invalid_leaf_then_revalidates_batch(self) -> None:
        reports = {
            producer: identity_report(f"{index + 1:064x}", producer=producer)
            for index, producer in enumerate(profiles.RUNTIME_SCRIPT_PROFILES)
        }
        invalid = min(profiles.RUNTIME_SCRIPT_PROFILES)
        validation = {
            "schema": "krr-runtime-script-validation-v1",
            "producer": invalid,
            "valid": True,
        }
        with (
            patch.object(
                identity_collector,
                "collect_fixed_leaf_identity_reports",
                return_value=reports,
            ) as collect,
            patch.object(
                cache_cli,
                "_validate_report",
                side_effect=[result.RuntimeScriptResultError("stale")]
                + [validation] * (len(reports) - 1)
                + [validation] * len(reports),
            ),
            patch.object(cache_cli, "run_current", return_value={"producer": invalid}) as run,
        ):
            result_report = cache_cli.ensure_all(self.root)
        self.assertTrue(result_report["recovered"])
        self.assertEqual(run.call_args.args, (self.root, invalid))
        self.assertEqual(collect.call_count, 2)

    def test_unavailable_collector_fails_closed_without_creating_anchor(self) -> None:
        with patch.object(
            identity_collector,
            "collect_identity_report",
            return_value=identity_report(cacheable=False),
        ), self.assertRaisesRegex(result.RuntimeScriptResultError, "collector_unavailable"):
            cache_cli.current_key(self.root)

        anchor_dir = self.git_directory() / local_anchor.DIRECTORY_NAME
        self.assertFalse(anchor_dir.exists())

    def test_run_current_invalidates_anchor_when_source_changes_after_run(self) -> None:
        run_identity = result.build_identity(PRODUCER, "e" * 64)
        reports = [identity_report(run_identity), identity_report("f" * 64)]
        with (
            patch.object(identity_collector, "collect_execution_snapshot", side_effect=reports),
            patch.object(recipe_reader, "read_runtime_script_recipe", return_value=self.recipe()),
            patch.object(producer_runner, "run_runtime_script", side_effect=self.fake_run(run_identity)),
            self.assertRaisesRegex(result.RuntimeScriptResultError, "changed during execution"),
        ):
            cache_cli.run_current(self.root)

        anchor_dir = self.git_directory() / local_anchor.DIRECTORY_NAME
        current = local_anchor._read(anchor_dir, PRODUCER)
        self.assertIsNotNone(current)
        self.assertEqual(current["state"] if current else None, "pending")
        current_epoch = str(current["epoch"] if current else "")
        self.assertFalse(local_anchor.verify(self.root, PRODUCER, run_identity, current_epoch, b"fresh-success"))

    def run_ensure_after_corrupt_receipt(
        self, postverify_error: str | None
    ) -> tuple[dict[str, object] | None, int, str | None]:
        run_identity = result.build_identity(PRODUCER, "e" * 64)
        postverify = (
            result.RuntimeScriptResultError(postverify_error)
            if postverify_error is not None
            else {"producer": PRODUCER}
        )
        with (
            patch.object(identity_collector, "collect_identity_report", return_value=identity_report(run_identity)),
            patch.object(identity_collector, "collect_execution_snapshot", return_value=identity_report(run_identity)),
            patch.object(recipe_reader, "read_runtime_script_recipe", return_value=self.recipe()),
            patch.object(producer_runner, "run_runtime_script", side_effect=self.fake_run(run_identity)),
            patch.object(
                result_verify,
                "verify_receipt",
                side_effect=[result.RuntimeScriptResultError("corrupt receipt"), postverify],
            ) as verify,
        ):
            try:
                report = cache_cli.ensure_current(self.root)
            except result.RuntimeScriptResultError as error:
                return None, verify.call_count, str(error)
        return report, verify.call_count, None

    def test_ensure_recovers_corrupt_receipt_and_verifies_fresh_producer(self) -> None:
        report, verify_count, error = self.run_ensure_after_corrupt_receipt(None)

        self.assertIsNone(error)
        if report is None:
            self.fail("ensure did not recover the corrupt receipt")
        self.assertTrue(report["valid"])
        self.assertTrue(report["recovered"])
        self.assertEqual(verify_count, 2)

    def test_ensure_invalidates_fresh_anchor_when_postverify_fails(self) -> None:
        report, verify_count, error = self.run_ensure_after_corrupt_receipt("fresh receipt rejected")

        self.assertIsNone(report)
        self.assertEqual(verify_count, 2)
        self.assertEqual(error, "fresh receipt rejected")
        anchor_dir = self.git_directory() / local_anchor.DIRECTORY_NAME
        current = local_anchor._read(anchor_dir, PRODUCER)
        self.assertIsNotNone(current)
        self.assertEqual(current["state"] if current else None, "pending")


if __name__ == "__main__":
    unittest.main()
