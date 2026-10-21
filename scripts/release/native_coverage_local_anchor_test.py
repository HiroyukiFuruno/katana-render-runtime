from __future__ import annotations

import json
import os
import stat
import tempfile
import unittest
from collections.abc import Callable
from pathlib import Path
from unittest.mock import patch

import native_coverage_local_anchor as anchor
import native_coverage_local_result as local_result
import native_coverage_result as coverage_result
import native_coverage_result_test as fixture_helpers
import runtime_script_profiles as profiles


class NativeCoverageLocalAnchorTests(unittest.TestCase):
    def setUp(self) -> None:
        self._reset_fixture_repository()

    def _reset_fixture_repository(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        fixture_helpers.initialize_fixture_repository(self.root)

    def test_private_anchor_epoch_is_stable_then_rotates(self) -> None:
        first = anchor.prepare(self.root, "root", "a" * 64)
        self.assertIsNotNone(first)
        self.assertEqual(anchor.prepare(self.root, "root", "a" * 64), first)
        self.assertTrue(
            anchor.commit(self.root, "root", "a" * 64, first or "", b"receipt")
        )
        self.assertTrue(
            anchor.verify(self.root, "root", "a" * 64, first or "", b"receipt")
        )
        anchor.invalidate(self.root, "root", "a" * 64, first or "")
        second = anchor.prepare(self.root, "root", "a" * 64)
        self.assertNotEqual(first, second)
        anchor_path = self.root / ".git" / anchor.DIRECTORY_NAME / "root.json"
        self.assertEqual(stat.S_IMODE(anchor_path.stat().st_mode), 0o600)
        self.assertEqual(stat.S_IMODE(anchor_path.parent.stat().st_mode), 0o700)

    def test_fixed_runtime_script_leaf_anchors_are_separate_from_coverage(self) -> None:
        expected_files = {
            "root": "root.json",
            "root-nextest-retry": "root-nextest-retry.json",
            "performance": "performance.json",
            "runtime-asset-script-other": "runtime-asset-script-other.json",
            "runtime-unused-script-regression": "runtime-unused-script-regression.json",
            **{producer: f"{producer}.json" for producer in profiles.RUNTIME_SCRIPT_PROFILES},
        }
        self.assertEqual(anchor.PRODUCERS, expected_files)
        leaf_anchors: dict[str, tuple[str, str, bytes]] = {}
        for index, producer in enumerate(expected_files):
            identity = f"{index:064x}"
            receipt = f"receipt:{producer}".encode()
            epoch = anchor.prepare(self.root, producer, identity)
            self.assertIsNotNone(epoch)
            self.assertTrue(anchor.commit(self.root, producer, identity, epoch or "", receipt))
            self.assertTrue(anchor.verify(self.root, producer, identity, epoch or "", receipt))
            self.assertFalse(anchor.verify(self.root, producer, identity, epoch or "", b"other receipt"))
            leaf_anchors[producer] = (identity, epoch or "", receipt)

        anchor_directory = self.root / ".git" / anchor.DIRECTORY_NAME
        self.assertEqual(set(os.listdir(anchor_directory)), set(expected_files.values()))
        root_identity, root_epoch, _ = leaf_anchors["root"]
        leaf_identity, leaf_epoch, leaf_receipt = leaf_anchors["runtime-asset-script-other"]
        self.assertFalse(
            anchor.verify(
                self.root,
                "root",
                leaf_identity,
                leaf_epoch,
                leaf_receipt,
            )
        )
        self.assertFalse(
            anchor.verify(
                self.root,
                "runtime-asset-script-other",
                root_identity,
                root_epoch,
                leaf_receipt,
            )
        )
        with self.assertRaisesRegex(ValueError, "invalid local coverage anchor identity"):
            anchor.prepare(self.root, "runtime-script", "e" * 64)

        (anchor_directory / "unexpected.json").write_text("{}", encoding="utf-8")
        self.assertIsNone(anchor.prepare(self.root, "runtime-unused-script-regression", "d" * 64))

    def test_root_collection_begin_invalidates_success_without_rotating_epoch(self) -> None:
        identity = "c" * 64
        epoch = anchor.prepare(self.root, "root", identity)
        self.assertIsNotNone(epoch)
        previous_receipt = b"previous root collection"
        self.assertTrue(anchor.commit(self.root, "root", identity, epoch or "", previous_receipt))
        self.assertTrue(anchor.verify(self.root, "root", identity, epoch or "", previous_receipt))

        self.assertTrue(anchor.begin_root_collection(self.root, identity, epoch or ""))
        self.assertEqual(anchor.prepare(self.root, "root", identity), epoch)
        self.assertFalse(anchor.verify(self.root, "root", identity, epoch or "", previous_receipt))

        next_receipt = b"new root collection"
        self.assertTrue(anchor.commit(self.root, "root", identity, epoch or "", next_receipt))
        self.assertTrue(anchor.verify(self.root, "root", identity, epoch or "", next_receipt))

    def test_runtime_script_begin_revokes_success_without_rotating_key_epoch(self) -> None:
        for producer in (
            "runtime-asset-script-other",
            "runtime-unused-script-regression",
        ):
            with self.subTest(producer=producer):
                identity = "b" * 64
                epoch = anchor.prepare(self.root, producer, identity) or ""
                self.assertTrue(anchor.commit(self.root, producer, identity, epoch, b"old"))
                self.assertFalse(anchor.begin_runtime_script(self.root, producer, "c" * 64, epoch))
                self.assertFalse(anchor.begin_runtime_script(self.root, producer, identity, "d" * 64))
                self.assertTrue(anchor.verify(self.root, producer, identity, epoch, b"old"))
                self.assertTrue(anchor.begin_runtime_script(self.root, producer, identity, epoch))
                self.assertEqual(anchor.prepare(self.root, producer, identity), epoch)
                self.assertFalse(anchor.verify(self.root, producer, identity, epoch, b"old"))
                self.assertTrue(anchor.commit(self.root, producer, identity, epoch, b"new"))
                self.assertTrue(anchor.verify(self.root, producer, identity, epoch, b"new"))
        with self.assertRaisesRegex(ValueError, "invalid runtime-script anchor producer"):
            anchor.begin_runtime_script(self.root, "root", "b" * 64, "d" * 64)

    @unittest.skipUnless(os.name == "posix", "private anchor permissions require POSIX")
    def test_symlink_and_nonprivate_directory_are_never_cacheable(self) -> None:
        git_directory = self.root / ".git"
        anchor_path = git_directory / anchor.DIRECTORY_NAME
        outside = self.root / "outside"
        outside.mkdir(mode=0o700)
        anchor_path.symlink_to(outside, target_is_directory=True)
        self.assertIsNone(anchor.prepare(self.root, "root", "b" * 64))
        anchor_path.unlink()
        anchor_path.mkdir(mode=0o755)
        anchor_path.chmod(0o755)
        self.assertIsNone(anchor.prepare(self.root, "root", "b" * 64))

    @unittest.skipUnless(os.name == "posix", "private anchor permissions require POSIX")
    def test_hardlinked_anchor_file_is_never_cacheable(self) -> None:
        directory = self.root / ".git" / anchor.DIRECTORY_NAME
        directory.mkdir(mode=0o700)
        record = anchor._pending("root", "c" * 64)
        anchor_path = directory / "root.json"
        self.assertTrue(anchor._write(directory, record))
        (self.root / "linked-anchor.json").hardlink_to(anchor_path)
        self.assertIsNone(anchor.prepare(self.root, "root", "c" * 64))
        (self.root / "linked-anchor.json").unlink()
        anchor_path.chmod(0o644)
        self.assertIsNone(anchor.prepare(self.root, "root", "c" * 64))

    @unittest.skipUnless(os.name == "posix", "private anchor permissions require POSIX")
    def test_unsafe_anchor_uses_fresh_only_probe_and_never_reuses_result(self) -> None:
        for domain in ("root", "baseline", "candidate"):
            fixture_helpers.write_fixture_report(self.root, domain)
        outside = self.root / "outside"
        outside.mkdir(mode=0o700)
        (self.root / ".git" / anchor.DIRECTORY_NAME).symlink_to(
            outside, target_is_directory=True
        )
        context = self._make_context()
        with (
            patch.object(
                local_result, "_source_state", return_value=self._make_source_state()
            ),
            patch.object(local_result, "_local_context", return_value=context),
            patch.dict(os.environ, {"MOON_TASK_ID": "krr:unit-test"}, clear=True),
        ):
            first = local_result.local_key(self.root, "root")
            self.assertFalse(first["cacheable"])
            local_result.local_begin(self.root, "root")
            local_result.local_finish(self.root, "root", 0)
            self.assertTrue(local_result.local_validate(self.root, "root")["valid"])
            local_result.local_key(self.root, "root")
            with self.assertRaises(local_result.LocalCoverageError):
                local_result.local_validate(self.root, "root")

    @staticmethod
    def _make_source_state() -> dict[str, str]:
        return {
            "schema": local_result.SOURCE_INPUTS_SCHEMA,
            "sourceDigest": "a" * 64,
            "coverageInputDigest": "b" * 64,
        }

    def _make_context(self) -> dict[str, object]:
        context: dict[str, object] = {
            "schema": local_result.LOCAL_CONTEXT_SCHEMA,
            "cacheable": True,
            "missReasons": [],
            "host": {
                "os": "macos",
                "productVersion": "15",
                "buildVersion": "24",
                "architecture": "arm64",
            },
            "tools": {key: f"{key}-1" for key in local_result.LOCAL_TOOL_KEYS},
            "environmentSha256": "d" * 64,
            "resourcesSha256": "e" * 64,
        }
        context["hostContextSha256"] = local_result._digest(context)
        return context

    def _rewrite_producer_outputs(self, producer: str) -> None:
        receipt_path = (
            self.root
            / coverage_result.REPORT_DIRECTORY
            / f"{producer}-local-result.json"
        )
        receipt = json.loads(receipt_path.read_bytes())
        domains = local_result._producer_domains(producer)
        report_names = {
            "root": "root.json",
            "performance-baseline": "baseline.json",
            "performance-candidate": "candidate.json",
        }
        for domain in domains:
            report_path = (
                self.root / coverage_result.REPORT_DIRECTORY / report_names[domain]
            )
            report = json.loads(report_path.read_bytes())
            report["version"] = "2.0.2"
            report_path.write_text(json.dumps(report), encoding="utf-8")
        receipt["domains"] = local_result._collection_report_records(self.root, producer)
        receipt_path.write_text(
            json.dumps(receipt, sort_keys=True) + "\n", encoding="utf-8"
        )

    def _run_and_attack(self, producer: str) -> None:
        self._assert_corruption_requires_fresh_run(
            producer,
            lambda: self._rewrite_producer_outputs(producer),
            "anchor",
        )

    def _rewrite_reports_only(self, producer: str) -> None:
        for domain in local_result._producer_domains(producer):
            report = self.root / coverage_result.REPORT_DIRECTORY / (
                local_result.REPORT_NAMES[domain]
            )
            report.write_bytes(report.read_bytes() + b" ")

    def _assert_corruption_requires_fresh_run(
        self, producer: str, corrupt: Callable[[], None], error: str
    ) -> None:
        key = local_result.local_key(self.root, producer)
        local_result.local_begin(self.root, producer)
        local_result.local_finish(self.root, producer, 0)
        corrupt()
        with self.assertRaisesRegex(local_result.LocalCoverageError, error):
            local_result.local_validate(self.root, producer)
        fresh_key = local_result.local_key(self.root, producer)
        self.assertNotEqual(key["fingerprint"], fresh_key["fingerprint"])
        local_result.local_begin(self.root, producer)
        local_result.local_finish(self.root, producer, 0)
        self.assertTrue(local_result.local_validate(self.root, producer)["valid"])

    def _with_valid_fixture_context(
        self, producer: str, exercise: Callable[[str], None]
    ) -> None:
        for domain in ("root", "baseline", "candidate"):
            fixture_helpers.write_fixture_report(self.root, domain)
        with (
            patch.object(
                local_result, "_source_state", return_value=self._make_source_state()
            ),
            patch.object(
                local_result, "_local_context", return_value=self._make_context()
            ),
            patch.dict(os.environ, {}, clear=True),
        ):
            exercise(producer)

    def test_coordinated_producer_report_and_receipt_rewrite_requires_fresh_run(
        self,
    ) -> None:
        for index, producer in enumerate(("root", "performance")):
            with self.subTest(producer=producer):
                if index > 0:
                    self._reset_fixture_repository()
                self._with_valid_fixture_context(producer, self._run_and_attack)

    def test_report_only_corruption_rotates_both_producer_epochs(self) -> None:
        for index, producer in enumerate(("root", "performance")):
            with self.subTest(producer=producer):
                if index > 0:
                    self._reset_fixture_repository()
                self._with_valid_fixture_context(
                    producer,
                    lambda selected: self._assert_corruption_requires_fresh_run(
                        selected,
                        lambda: self._rewrite_reports_only(selected),
                        "coverage reports changed",
                    ),
                )


if __name__ == "__main__":
    unittest.main()
