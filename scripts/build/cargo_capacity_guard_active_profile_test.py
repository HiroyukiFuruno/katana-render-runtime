from __future__ import annotations

import json
import os
from pathlib import Path
import tempfile
import unittest

from cargo_capacity_guard import workspace_incremental_prefixes
from cargo_capacity_guard_active_profile_fixtures import (
    active_files, allocated_bytes, capacity_report, cargo_env, cargo_messages, fixture, fixture_state,
    guard_call, identities, protect_outputs, real_cargo, size_bytes, workspace_metadata,
)
from cargo_capacity_guard_lock import ProfileLocks, evict_artifacts, retained_profile_paths


@unittest.skipIf(os.name == "nt", "POSIX native locking and allocated-byte fixtures")
class ActiveProfileTests(unittest.TestCase):
    def assert_warm_release(self, root: Path, active: list[Path], phase: str) -> list[str]:
        before = identities(active_files(active))
        scope = active[0].parent
        stdout = real_cargo(root, ["build", "--release", "--offline", "--message-format=json"],
                            cargo_env(scope, scope / "build"))
        rows = cargo_messages(stdout)
        artifacts = [row for row in rows if row.get("reason") == "compiler-artifact"]
        self.assertTrue(artifacts, "actual compiler-artifact JSON is required")
        self.assertTrue(all(row["fresh"] is True for row in artifacts), artifacts)
        after = identities(active_files(active))
        self.assertEqual([(r[0], r[1], r[3], r[4]) for r in before],
                         [(r[0], r[1], r[3], r[4]) for r in after])
        delta = [str(Path(old[0]).relative_to(root))
                 for index, old in enumerate(before) if old[2] != after[index][2]]
        destination = os.environ.get("CAPACITY_FIXTURE_LOG_DIR")
        if destination:
            path = Path(destination) / (root.name + "-" + phase + ".json")
            path.write_text(json.dumps({"before": before, "after": after, "inodeOnlyChanges": delta,
                                         "compilerArtifacts": artifacts}, sort_keys=True) + "\n")
        return delta

    def real_workspace_closure(self, inactive: Path) -> tuple[Path, list[Path]]:
        objects = list((inactive / "deps").glob("helper-*.rcgu.o"))
        self.assertTrue(objects, "actual helper Cargo object required")
        artifact = objects[0]
        linked = [p for p in (inactive / "incremental").rglob("*.o")
                  if p.stat().st_ino == artifact.stat().st_ino]
        self.assertTrue(linked, "real deps/incremental inode alias required")
        os.utime(artifact, ns=(1, 1))
        return artifact, linked

    def test_real_workspace_inactive_closure_keeps_active_profiles_and_15_outputs(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target, inactive, active, _, prefixes, _ = fixture(root)
            self.assertEqual(prefixes, ["consumer-", "helper-"])
            unmodified_warm_delta = self.assert_warm_release(root, active, "no-eviction-control")
            refs = protect_outputs(root, [inactive, active[1]])
            old_refs = identities(refs)
            artifact, links = self.real_workspace_closure(inactive)
            with ProfileLocks(target) as epoch:
                old_active = identities(active_files(active))
                before = size_bytes(target)
                baseline = (before - 2) // 2
                code, report = evict_artifacts(target, baseline * 2, before, False, refs, epoch,
                                               incremental_prefixes=["consumer-"],
                                               inactive_prefixes=prefixes, retained_profiles=active)
                self.assertEqual(code, 0, report)
                self.assertEqual(identities(active_files(active)), old_active)
            self.assertFalse(artifact.exists())
            self.assertTrue(all(not p.exists() for p in links))
            self.assertTrue(any("/incremental/helper-" in p for p in report["candidate_paths"]))
            self.assertTrue(all(p.startswith("inactive/debug/") for p in report["candidate_paths"]))
            self.assertLessEqual(size_bytes(target), baseline * 2)
            self.assertEqual(identities(refs), old_refs)
            warm_delta = self.assert_warm_release(root, active, "after-inactive-eviction")
            self.assertEqual(warm_delta, unmodified_warm_delta)
            self.assertEqual(identities(refs), old_refs)

    def test_real_retained_floor_above_fixed_threshold_blocks_child_without_cleanup(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target, inactive, active, baseline, _, _ = fixture(root)
            refs = protect_outputs(root, [inactive, active[1]])
            retained = [*active, inactive]
            old = identities([*active_files(retained), *refs])
            self.assertGreater(size_bytes(target), baseline * 2)
            sentinel = root / "child-ran"
            code, report, state = guard_call(root, target, retained, refs, baseline, sentinel)
            self.assertEqual(code, 1, report)
            self.assertFalse(sentinel.exists())
            self.assertEqual(report["threshold_bytes"], baseline * 2)
            self.assertEqual(report["current_bytes"], report["post_clean_bytes"])
            self.assertEqual(report["finite_cleanup"]["deleted_files"], 0)
            self.assertIn("insufficient closed inactive", report["finite_cleanup"]["error"])
            self.assertEqual(identities([*active_files(retained), *refs]), old)
            self.assertEqual(state["baseline_bytes"], baseline)
            below_baseline = size_bytes(target)
            code, report, _ = guard_call(root, target, retained, refs, below_baseline, sentinel)
            self.assertEqual(code, 0, report)
            self.assertEqual(report["cleanup_status"], "not-needed")
            self.assertEqual(report["actual_removed_bytes"], 0)
            self.assertTrue(sentinel.exists())

    def test_real_active_cross_alias_and_unknown_alias_are_retained(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target, inactive, active, _, prefixes, _ = fixture(root)
            refs = protect_outputs(root, [inactive, active[1]])
            deps = list((inactive / "deps").glob("*.d"))
            self.assertGreaterEqual(len(deps), 2)
            active_alias = active[1] / "deps" / deps[0].name
            self.assertFalse(active_alias.exists())
            unknown_alias = root / "unmanaged-link"
            os.link(deps[0], active_alias)
            os.link(deps[1], unknown_alias)
            os.utime(deps[0], ns=(1, 1))
            os.utime(deps[1], ns=(2, 2))
            old = identities([*deps, active_alias, unknown_alias, *refs])
            with ProfileLocks(target) as epoch:
                old_active = identities(active_files(active))
                before = size_bytes(target)
                code, report = evict_artifacts(target, before - 1, before, False, refs, epoch,
                                               inactive_prefixes=prefixes, retained_profiles=active)
                self.assertEqual(code, 0, report)
                self.assertEqual(identities(active_files(active)), old_active)
            self.assertGreater(report["deleted_files"], 0)
            self.assertGreaterEqual(report["retained_unknown_alias_inodes"], 1)
            self.assertGreaterEqual(report["retained_active_closures"], 1)
            self.assertEqual(identities([*deps, active_alias, unknown_alias, *refs]), old)

    def test_metadata_scope_and_retained_path_validation_are_bounded(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            target, _, active, _, prefixes, meta = fixture(root)
            outsider = dict(meta["packages"][0], id="outside", manifest_path=str(root.parent / "outside/Cargo.toml"))
            outsider["targets"] = [{"name": "not_owned", "kind": ["lib"]}]
            supplied = dict(meta, packages=[*meta["packages"], outsider],
                            workspace_members=[*meta["workspace_members"], "outside"])
            self.assertEqual(workspace_incremental_prefixes(root, supplied), prefixes)
            bad = ["relative/release", str(root / "release"), str(target / "active/build/../release")]
            old = identities(active_files(active))
            for value in bad:
                with self.subTest(value=value), self.assertRaises(RuntimeError):
                    retained_profile_paths(target, [value])
            (target / "alias").symlink_to(active[0].parent, target_is_directory=True)
            with self.assertRaisesRegex(RuntimeError, "kind or device boundary"):
                retained_profile_paths(target, [str(target / "alias/release")])
            self.assertEqual(identities(active_files(active)), old)


class ActiveProfileBoundaryUnitTests(unittest.TestCase):
    def test_workspace_metadata_shape_preserves_native_payload(self) -> None:
        valid: dict[str, object] = {"packages": [{"id": "helper", "extra": 7}],
                                   "workspace_members": ["helper"], "nativeExtra": [1, 2]}
        self.assertIs(workspace_metadata(valid), valid)
        self.assertEqual(workspace_metadata(valid)["packages"][0]["extra"], 7)
        invalid: list[object] = [None, [], {1: "bad"}, {},
                                 {**valid, "packages": {}}, {**valid, "packages": [None]},
                                 {**valid, "packages": [{1: "bad"}]},
                                 {**valid, "workspace_members": "helper"},
                                 {**valid, "workspace_members": [True]}]
        for value in invalid:
            with self.subTest(value=value), self.assertRaises((RuntimeError, KeyError)):
                workspace_metadata(value)

    def test_cargo_message_shape_preserves_fresh_boolean(self) -> None:
        valid: dict[str, object] = {"reason": "compiler-artifact", "fresh": True,
                                   "nativeExtra": {"name": "helper"}}
        self.assertEqual(cargo_messages(json.dumps(valid)), [valid])
        self.assertEqual(cargo_messages('{"reason":"build-finished","success":true}'),
                         [{"reason": "build-finished", "success": True}])
        invalid_fresh: tuple[object, ...] = (1, "true", None, [])
        invalid: list[object] = [[], None, {}, {"reason": True},
                                 {"reason": "compiler-artifact"},
                                 *[{**valid, "fresh": value} for value in invalid_fresh]]
        for value in invalid:
            with self.subTest(value=value), self.assertRaises((RuntimeError, KeyError)):
                cargo_messages(json.dumps(value))

    def test_capacity_report_state_and_allocated_bytes_reject_coercion(self) -> None:
        valid: dict[str, object] = {"threshold_bytes": 4726095872.0, "current_bytes": 7,
                                   "post_clean_bytes": 7, "actual_removed_bytes": 0,
                                   "cleanup_status": "blocked",
                                   "finite_cleanup": {"deleted_files": 0, "error": "closed inactive"},
                                   "nativeExtra": [1, 2]}
        self.assertIs(capacity_report(valid), valid)
        nullable_threshold = {key: value for key, value in valid.items() if key != "finite_cleanup"}
        nullable_threshold.update(threshold_bytes=None, post_clean_bytes=None, cleanup_status="uncalibrated")
        self.assertIs(capacity_report(nullable_threshold), nullable_threshold)
        nullable_post_clean = {key: value for key, value in valid.items() if key != "finite_cleanup"}
        nullable_post_clean.update(post_clean_bytes=None, cleanup_status="not-needed")
        self.assertIs(capacity_report(nullable_post_clean), nullable_post_clean)
        state: dict[str, object] = {"baseline_bytes": 2363047936, "nativeExtra": "preserved"}
        self.assertIs(fixture_state(state), state)
        for field in ("threshold_bytes", "current_bytes", "post_clean_bytes", "actual_removed_bytes"):
            invalid_values: tuple[object, ...] = (True, "7", [], float("nan"))
            if field in {"current_bytes", "actual_removed_bytes"}:
                invalid_values = (*invalid_values, None)
            for value in invalid_values:
                with self.subTest(field=field, value=value), self.assertRaises(RuntimeError):
                    capacity_report({**valid, field: value})
        for value in ([], {"deleted_files": True}, {"deleted_files": 0, "error": 7}):
            with self.subTest(cleanup=value), self.assertRaises(RuntimeError):
                capacity_report({**valid, "finite_cleanup": value})
        self.assertEqual(capacity_report({**valid, "finite_cleanup": {"deleted_files": 0}})["finite_cleanup"]["deleted_files"], 0)
        for value in (True, "2363047936", None, 1.0):
            with self.subTest(baseline=value), self.assertRaises(RuntimeError):
                fixture_state({"baseline_bytes": value})
        self.assertEqual(allocated_bytes(0), 0)
        self.assertEqual(allocated_bytes(7), 7)
        for value in (None, True, "7", 7.0, -1):
            with self.subTest(allocated=value), self.assertRaises(AssertionError):
                allocated_bytes(value)


if __name__ == "__main__":
    unittest.main()
