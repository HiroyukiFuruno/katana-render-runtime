"""End-to-end tests for the fixed Bun runtime-script producer."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

import native_coverage_local_anchor as anchor
import runtime_script_producer as producer
import runtime_script_profiles as profiles
import runtime_script_recipe as recipe
import runtime_script_result_verify as result_verify


class RuntimeScriptProducerTests(unittest.TestCase):
    def _repo(self) -> tempfile.TemporaryDirectory[str]:
        temporary = tempfile.TemporaryDirectory()
        subprocess.run(["git", "init", "--quiet"], cwd=temporary.name, check=True)
        root = Path(temporary.name)
        fixture = root / "scripts/runtime-bundles/one_test.ts"
        fixture.parent.mkdir(parents=True)
        fixture.write_text(
            'import { expect, test } from "bun:test";\n'
            'test("producer", () => expect(1).toBe(1));\n',
            encoding="utf-8",
        )
        return temporary

    def _recipe(self) -> recipe.RuntimeScriptRecipe:
        command = (
            "bun",
            "test",
            "--path-ignore-patterns",
            "tmp/**",
            "./scripts/runtime-bundles/one_test.ts",
        )
        return recipe.RuntimeScriptRecipe(
            "runtime-unused-script-regression-run", "a" * 64, (command,)
        )

    def _run_profile_success(
        self, repo: Path, producer_name: str, test_path: str
    ) -> producer.RuntimeScriptRun:
        fixture = repo / test_path.removeprefix("./")
        fixture.parent.mkdir(parents=True, exist_ok=True)
        fixture.write_text(
            'import { expect, test } from "bun:test";\n'
            'test("profile producer", () => expect(1).toBe(1));\n',
            encoding="utf-8",
        )
        command = ("bun", "test", "--path-ignore-patterns", "tmp/**", test_path)
        recipe_value = recipe.RuntimeScriptRecipe(
            f"{producer_name}-run", "a" * 64, (command,)
        )
        run = producer.run_runtime_script(repo, producer_name, "b" * 64, recipe_value)
        receipt_path = repo / run.receipt_path
        self.assertEqual(run.process_exit, 0)
        self.assertEqual(len(run.reports), 1)
        self.assertTrue(
            result_verify.verify_receipt(
                repo, producer_name, run.identity_sha256, run.epoch, receipt_path
            )
        )
        raw = repo / run.raw_log_directory
        self.assertEqual((raw / "group-001-exit.txt").read_text(), "0\n")
        actual_command = json.loads((raw / "group-001-command.json").read_text())["argv"]
        self.assertEqual(actual_command[-2], "--reporter-outfile")
        self.assertRegex(
            actual_command[-1],
            rf"tmp/quality/runtime-script-results/{producer_name}-[0-9a-f]{{64}}-group-001.xml",
        )
        return run

    def test_real_bun_generates_junit_and_commits_receipt(self) -> None:
        with self._repo() as directory:
            repo = Path(directory).resolve()
            run = producer.run_runtime_script(
                repo,
                "runtime-unused-script-regression",
                "b" * 64,
                self._recipe(),
            )
            receipt_path = repo / run.receipt_path
            self.assertTrue(receipt_path.is_file())
            self.assertEqual(run.process_exit, 0)
            self.assertEqual(len(run.reports), 1)
            self.assertTrue(
                result_verify.verify_receipt(
                    repo, run.producer, run.identity_sha256, run.epoch, receipt_path
                )
            )
            first_receipt = receipt_path.read_bytes()
            first_raw = run.raw_log_directory
            second = producer.run_runtime_script(
                repo,
                "runtime-unused-script-regression",
                "b" * 64,
                self._recipe(),
            )
            self.assertEqual(second.epoch, run.epoch)
            self.assertNotEqual(second.raw_log_directory, first_raw)
            self.assertTrue(
                result_verify.verify_receipt(
                    repo,
                    second.producer,
                    second.identity_sha256,
                    second.epoch,
                    receipt_path,
                )
            )
            self.assertTrue(first_receipt)
            raw = repo / second.raw_log_directory
            self.assertEqual((raw / "group-001-exit.txt").read_text(), "0\n")
            command = json.loads((raw / "group-001-command.json").read_text())["argv"]
            self.assertEqual(command[-2], "--reporter-outfile")
            self.assertRegex(
                command[-1],
                r"tmp/quality/runtime-script-results/runtime-unused-script-regression-[0-9a-f]{64}-group-001.xml",
            )

    def test_all_fixed_profiles_run_real_bun_and_commit_separate_receipts(self) -> None:
        with self._repo() as directory:
            repo = Path(directory).resolve()
            for producer_name, test_path in profiles.RUNTIME_SCRIPT_PROFILES.items():
                with self.subTest(producer=producer_name):
                    self._run_profile_success(repo, producer_name, test_path)

    def test_two_quality_profiles_run_real_bun_and_commit_separate_receipts(self) -> None:
        with self._repo() as directory:
            repo = Path(directory).resolve()
            for producer_name, test_path in profiles.QUALITY_SCRIPT_PROFILES.items():
                with self.subTest(producer=producer_name):
                    self._run_profile_success(repo, producer_name, test_path)

    def test_quality_profile_inventory_rejects_missing_other_and_extra_files(self) -> None:
        producer_name = "runtime-quality-owned-source-inventory"
        command_prefix = ("bun", "test", "--path-ignore-patterns", "tmp/**")
        invalid_commands = (
            command_prefix,
            (*command_prefix, "./scripts/quality/owned-validation-contract_test.ts"),
            (
                *command_prefix,
                profiles.QUALITY_SCRIPT_PROFILES[producer_name],
                "./scripts/runtime-bundles/extra_test.ts",
            ),
        )
        for command in invalid_commands:
            with self.subTest(command=command), self.assertRaisesRegex(
                producer.RuntimeScriptProducerError, "inventory"
            ):
                producer._expected_files(command, producer_name)

    def test_expected_files_requires_dot_slash_for_default_and_all_fixed_profiles(self) -> None:
        profile_pairs = (
            (profiles.DEFAULT_PRODUCER, "./scripts/runtime-bundles/example_test.ts"),
            *profiles.RUNTIME_SCRIPT_PROFILES.items(),
            *profiles.QUALITY_SCRIPT_PROFILES.items(),
        )
        command_prefix = ("bun", "test", "--path-ignore-patterns", "tmp/**")
        for producer_name, accepted_path in profile_pairs:
            with self.subTest(producer=producer_name):
                self.assertEqual(
                    producer._expected_files((*command_prefix, accepted_path), producer_name),
                    (accepted_path.removeprefix("./"),),
                )
                with self.assertRaisesRegex(
                    producer.RuntimeScriptProducerError, "inventory is unsupported"
                ):
                    producer._expected_files(
                        (*command_prefix, accepted_path.removeprefix("./")), producer_name
                    )
                with self.assertRaisesRegex(
                    producer.RuntimeScriptProducerError, "inventory is unsupported"
                ):
                    producer._expected_files(
                        (*command_prefix, "/repo/" + accepted_path.removeprefix("./")),
                        producer_name,
                    )

    def test_fixed_profile_rejects_multiple_commands_before_identity_collection(self) -> None:
        producer_name, test_path = next(iter(profiles.QUALITY_SCRIPT_PROFILES.items()))
        command = ("bun", "test", "--path-ignore-patterns", "tmp/**", test_path)
        recipe_value = recipe.RuntimeScriptRecipe(
            f"{producer_name}-run", "a" * 64, (command, command)
        )
        with self._repo() as directory:
            repo = Path(directory).resolve()
            with self.assertRaisesRegex(producer.RuntimeScriptProducerError, "one command"):
                producer.run_runtime_script(repo, producer_name, "b" * 64, recipe_value)

    def test_cross_profile_inventory_is_rejected_before_bun(self) -> None:
        profiles_list = list(profiles.RUNTIME_SCRIPT_PROFILES.items())
        _, declared_path = profiles_list[0]
        other_name, _ = profiles_list[1]
        with self._repo() as directory:
            repo = Path(directory).resolve()
            fixture = repo / declared_path.removeprefix("./")
            fixture.parent.mkdir(parents=True, exist_ok=True)
            fixture.write_text(
                'import { expect, test } from "bun:test";\n'
                'test("cross profile", () => expect(1).toBe(1));\n',
                encoding="utf-8",
            )
            mismatched = recipe.RuntimeScriptRecipe(
                f"{other_name}-run",
                "a" * 64,
                (("bun", "test", "--path-ignore-patterns", "tmp/**", declared_path),),
            )
            with self.assertRaisesRegex(producer.RuntimeScriptProducerError, "inventory"):
                producer.run_runtime_script(repo, other_name, "b" * 64, mismatched)

    def test_same_profile_lease_rejects_second_run(self) -> None:
        with self._repo() as directory:
            repo = Path(directory).resolve()
            lease = repo / "tmp/quality/runtime-script-results/.runtime-unused-script-regression.lease"
            lease.parent.mkdir(parents=True)
            lease.parent.chmod(0o700)
            lease.write_bytes(b"held\n")
            with self.assertRaisesRegex(producer.RuntimeScriptProducerError, "already running"):
                producer.run_runtime_script(
                    repo,
                    "runtime-unused-script-regression",
                    "b" * 64,
                    self._recipe(),
                )

    def test_failed_fresh_run_invalidates_previous_receipt_and_keeps_raw_logs(self) -> None:
        with self._repo() as directory:
            repo = Path(directory).resolve()
            first = producer.run_runtime_script(
                repo,
                "runtime-unused-script-regression",
                "b" * 64,
                self._recipe(),
            )
            receipt_path = repo / first.receipt_path
            previous_receipt = receipt_path.read_bytes()
            fixture = repo / "scripts/runtime-bundles/one_test.ts"
            fixture.write_text(
                'import { expect, test } from "bun:test";\n'
                'test("producer", () => expect(1).toBe(2));\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(producer.RuntimeScriptProducerError, "exit 1"):
                producer.run_runtime_script(
                    repo,
                    "runtime-unused-script-regression",
                    "b" * 64,
                    self._recipe(),
                )
            self.assertFalse(
                anchor.verify(
                    repo,
                    first.producer,
                    first.identity_sha256,
                    first.epoch,
                    previous_receipt,
                )
            )
            raw_runs = sorted((repo / "tmp/quality/runtime-script-results/raw").glob("run-*"))
            failed_raw = next(
                run
                for run in raw_runs
                if (run / "group-001-exit.txt").read_text() == "1\n"
            )
            self.assertEqual((failed_raw / "group-001-exit.txt").read_text(), "1\n")
            self.assertTrue((failed_raw / "group-001-stdout.log").is_file())
            self.assertTrue((failed_raw / "group-001-stderr.log").is_file())
            command = json.loads((failed_raw / "group-001-command.json").read_text())["argv"]
            self.assertEqual(command[-2], "--reporter-outfile")


if __name__ == "__main__":
    unittest.main()
