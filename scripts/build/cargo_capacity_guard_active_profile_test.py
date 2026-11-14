from __future__ import annotations

import json
import os
from pathlib import Path
import tempfile
import unittest
import subprocess
from dataclasses import replace

from cargo_capacity_guard import JsonObject, _object, _selected_input_check
from cargo_capacity_guard_build_script_ownership import BuildScriptOwnership, build_script_ownership

from cargo_capacity_guard import workspace_incremental_prefixes
from cargo_capacity_guard_active_profile_fixtures import (
    active_files, allocated_bytes, capacity_report, cargo_env, cargo_messages, fixture, fixture_state,
    guard_call, identities, protect_outputs, real_cargo, size_bytes, workspace_metadata,
)
from cargo_capacity_guard_lock import ProfileLocks, evict_artifacts, retained_profile_paths
from cargo_capacity_guard_selected_cache_ownership import SelectedCacheOwnership, create_selected_cache_ownership


def protected_host_metadata(root: Path, target: Path) -> tuple[list[Path], JsonObject]:
    refs = [target / "debug/mathjax_consumer_facts"]
    frozen = root / "frozen"
    frozen.mkdir()
    for index in range(39):
        path = frozen / str(index)
        path.write_bytes(f"consumer {index}".encode())
        refs.append(path)
    metadata = _object(json.loads(real_cargo(root, ["metadata", "--offline", "--locked", "--all-features",
                                                   "--format-version", "1"], cargo_env(target))), "native host fixture")
    return refs, metadata


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


@unittest.skipIf(os.name == "nt", "POSIX native lock and allocated-byte Cargo fixture")
class OwnedBuildScriptNativeTests(unittest.TestCase):
    def workspace(self, root: Path) -> None:
        (root / "Cargo.toml").write_text('[workspace]\nmembers=["crates/kdr-linter","crates/worker"]\nresolver="2"\n')
        host = root / "crates/kdr-linter"
        worker = root / "crates/worker"
        for package in (host, worker):
            (package / "src").mkdir(parents=True)
        (host / "Cargo.toml").write_text('[package]\nname="kdr-linter"\nversion="0.1.0"\nedition="2021"\n'
                                         '[[bin]]\nname="mathjax_consumer_facts"\npath="src/main.rs"\n')
        (host / "src/main.rs").write_text('fn main() { println!("selected host"); }\n')
        (worker / "Cargo.toml").write_text('[package]\nname="worker"\nversion="0.1.0"\nedition="2021"\n'
                                           '[features]\nalternate=[]\n'
                                           '[[bin]]\nname="worker_probe"\npath="src/main.rs"\n')
        (worker / "src/lib.rs").write_text('include!(concat!(env!("OUT_DIR"), "/generated.rs"));\n')
        (worker / "src/main.rs").write_text('fn main() { println!("{}", worker::value()); }\n')
        (worker / "build.rs").write_text(
            'fn main() {\n'
            ' let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());\n'
            ' std::fs::write(out.join("generated.rs"), "pub fn value() -> u32 { 7 }\\n").unwrap();\n'
            ' println!("cargo:rerun-if-changed=build.rs");\n'
            '}\n',
        )

    def prepared(self, root: Path) -> tuple[Path, Path, list[Path], JsonObject]:
        self.workspace(root)
        target = root / "target"
        env = cargo_env(target)
        real_cargo(root, ["build", "--offline", "--message-format=json"], env)
        old = list((target / "debug/build").glob("worker-*/build-script-build"))
        self.assertEqual(len(old), 1)
        real_cargo(root, ["build", "--offline", "--features", "worker/alternate", "--message-format=json"], env)
        (root / "old-fingerprint").mkdir()
        fingerprint = target / "debug/.fingerprint" / old[0].parent.name
        fingerprint.rename(root / "old-fingerprint" / fingerprint.name)
        refs, metadata = protected_host_metadata(root, target)
        return target, old[0].parent, refs, metadata

    def test_real_orphan_script_closure_keeps_40_consumers_and_regenerates_missing_unit(self) -> None:
        with tempfile.TemporaryDirectory() as value:
            root = Path(value).resolve()
            target, orphan, refs, metadata = self.prepared(root)
            profile = target / "debug"
            proof = build_script_ownership(root, target, metadata, _selected_input_check(root, metadata))
            old_refs = identities(refs)
            fingerprint_files = active_files([profile / ".fingerprint"])
            fingerprints_before = identities(fingerprint_files)
            outputs = [path for path in (profile / "build").rglob("*")
                       if path.is_file() and not path.is_relative_to(orphan)]
            outputs_before = identities(outputs)
            orphan_files = sorted(orphan.iterdir())
            self.assertEqual(len(orphan_files), 3)
            available = sum(path.stat().st_blocks * 512 for path in orphan_files)
            before = size_bytes(target)
            with ProfileLocks(target, proof.verify) as epoch:
                code, preview = evict_artifacts(target, before - available, before, True, refs, epoch,
                                                retained_profiles=[profile], selected_package_profiles=[profile],
                                                build_script_ownership=proof)
                self.assertEqual(code, 0, preview)
                self.assertEqual(preview["deleted_files"], 0)
                self.assertEqual(preview["estimated_candidate_bytes"], available)
                self.assertEqual(identities(refs), old_refs)
                code, report = evict_artifacts(target, before - available, before, False, refs, epoch,
                                               retained_profiles=[profile], selected_package_profiles=[profile],
                                               build_script_ownership=proof)
                self.assertEqual(code, 0, report)
                self.assertEqual(report["deleted_files"], 3)
                self.assertEqual(report["invalidated_profiles"], [])
            self.assertFalse(list(orphan.iterdir()))
            self.assertEqual(before - size_bytes(target), available)
            self.assertEqual(identities(refs), old_refs)
            self.assertEqual(identities(fingerprint_files), fingerprints_before)
            self.assertEqual(identities(outputs), outputs_before)
            selected = cargo_messages(real_cargo(root, ["build", "--offline", "--locked", "-p", "kdr-linter",
                                                        "--bin", "mathjax_consumer_facts", "--message-format=json"],
                                                  cargo_env(target)))
            artifacts = [row for row in selected if row.get("reason") == "compiler-artifact"]
            self.assertTrue(artifacts)
            self.assertTrue(all(row["fresh"] for row in artifacts), artifacts)
            regenerated = cargo_messages(real_cargo(root, ["build", "--offline", "--locked", "-p", "worker",
                                                           "--message-format=json"], cargo_env(target)))
            self.assertTrue(any(row.get("reason") == "compiler-artifact" and row["fresh"] is False
                                for row in regenerated), regenerated)
            self.assertTrue((orphan / "build-script-build").is_file())
            completed = subprocess.run([str(profile / "worker_probe")], cwd=root, text=True,
                                       capture_output=True, check=False, timeout=10)
            self.assertEqual((completed.returncode, completed.stdout.strip()), (0, "7"))


@unittest.skipIf(os.name == "nt", "POSIX native session locking and allocated-byte Cargo fixture")
class SelectedCacheNativeTests(unittest.TestCase):
    def prepared(self, root: Path) -> tuple[Path, list[Path], JsonObject]:
        host = root / "crates/kdr-linter"
        (host / "src").mkdir(parents=True)
        (root / "Cargo.toml").write_text('[workspace]\nmembers=["crates/kdr-linter"]\nresolver="2"\n')
        (host / "Cargo.toml").write_text('[package]\nname="kdr-linter"\nversion="0.1.0"\nedition="2021"\n'
                                        '[[bin]]\nname="mathjax_consumer_facts"\npath="src/main.rs"\n')
        (host / "src/lib.rs").write_text('pub fn value() -> u32 { 7 }\n')
        (host / "src/main.rs").write_text('fn main() { println!("{}", kdr_linter::value()); }\n')
        target = root / "target"
        real_cargo(root, ["build", "--offline", "--message-format=json"], cargo_env(target))
        refs, metadata = protected_host_metadata(root, target)
        return target, refs, metadata

    def test_native_protected_or_unknown_alias_latest_index_refuses_all_deletion(self) -> None:
        for boundary in ("protected", "unknown-alias"):
            with self.subTest(boundary=boundary), tempfile.TemporaryDirectory() as value:
                root = Path(value).resolve()
                target, refs, metadata = self.prepared(root)
                profile = target / "debug"
                indexes = [p for p in (profile / "incremental").rglob("*")
                           if p.name in {"dep-graph.bin", "query-cache.bin"}]
                self.assertEqual(len(indexes), 4)
                selected = indexes[0]
                if boundary == "protected":
                    refs[-1] = selected
                else:
                    os.link(selected, root / "unknown-index-alias")
                old_indexes, old_refs = identities(indexes), identities(refs)
                input_check = _selected_input_check(root, metadata)
                original_orphan = build_script_ownership(root, target, metadata, input_check)
                original_cache = create_selected_cache_ownership(root, target, metadata, input_check)

                def verify(orphan: BuildScriptOwnership = original_orphan,
                           cache: SelectedCacheOwnership = original_cache) -> None:
                    orphan.verify()
                    cache.verify()

                before = size_bytes(target)
                available = sum(p.stat().st_blocks * 512 for p in indexes)
                with ProfileLocks(target, verify) as epoch:
                    code, report = evict_artifacts(
                        target, before - available, before, False, refs, epoch,
                        retained_profiles=[profile], selected_package_profiles=[profile],
                        build_script_ownership=replace(original_orphan, verify=verify),
                        selected_cache_ownership=replace(original_cache, verify=verify),
                    )
                self.assertEqual(code, 1, report)
                self.assertEqual(report["deleted_files"], 0)
                self.assertEqual(report["invalidated_profiles"], [])
                self.assertEqual(identities(indexes), old_indexes)
                self.assertEqual(identities(refs), old_refs)
                self.assertIn("insufficient closed inactive", report["error"])

    def test_native_latest_two_indexes_only_preserve_warm_skip_and_changed_source_rebuild(self) -> None:
        with tempfile.TemporaryDirectory() as value:
            root = Path(value).resolve()
            target, refs, metadata = self.prepared(root)
            profile = target / "debug"
            indexes = [p for p in (profile / "incremental").rglob("*")
                       if p.name in {"dep-graph.bin", "query-cache.bin"}]
            self.assertEqual(len(indexes), 4, "actual library and binary index pairs required")
            held = [p for p in profile.rglob("*") if p.is_file() and p not in indexes
                    and (p.suffix in {".o", ".rmeta", ".rlib", ".d"}
                         or p.name in {"work-products.bin", "mathjax_consumer_facts"}
                         or p.is_relative_to(profile / ".fingerprint"))]
            old_held, old_refs = identities(held), identities(refs)
            input_check = _selected_input_check(root, metadata)
            original_orphan = build_script_ownership(root, target, metadata, input_check)
            original_cache = create_selected_cache_ownership(root, target, metadata, input_check)

            def verify() -> None:
                original_orphan.verify()
                original_cache.verify()

            orphan = replace(original_orphan, verify=verify)
            cache = replace(original_cache, verify=verify)
            before = size_bytes(target)
            available = sum(p.stat().st_blocks * 512 for p in indexes)
            with ProfileLocks(target, verify) as epoch:
                code, preview = evict_artifacts(target, before - available, before, True, refs, epoch,
                                                retained_profiles=[profile], selected_package_profiles=[profile],
                                                build_script_ownership=orphan, selected_cache_ownership=cache)
                self.assertEqual(code, 0, preview)
                self.assertEqual(set(preview["candidate_paths"]), {str(p.relative_to(target)) for p in indexes})
                self.assertEqual(preview["deleted_files"], 0)
                code, report = evict_artifacts(target, before - available, before, False, refs, epoch,
                                               retained_profiles=[profile], selected_package_profiles=[profile],
                                               build_script_ownership=orphan, selected_cache_ownership=cache)
                self.assertEqual(code, 0, report)
                self.assertEqual(report["deleted_files"], 4)
                self.assertEqual(report["invalidated_profiles"], [])
            self.assertTrue(all(not p.exists() for p in indexes))
            self.assertEqual(identities(held), old_held)
            self.assertEqual(identities(refs), old_refs)
            self.assertEqual(before - size_bytes(target), available)
            build = ["build", "--offline", "--locked", "-p", "kdr-linter", "--bin", "mathjax_consumer_facts",
                     "--message-format=json"]
            warm = [r for r in cargo_messages(real_cargo(root, build, cargo_env(target)))
                    if r.get("reason") == "compiler-artifact"]
            self.assertTrue(warm)
            self.assertTrue(all(r["fresh"] is True for r in warm), warm)
            # Cargoのfreshな出力配置でinodeだけが変わる場合も、成果物の内容は維持する。
            self.assertEqual([(r[0], r[1], r[3], r[4]) for r in identities(held)],
                             [(r[0], r[1], r[3], r[4]) for r in old_held])
            self.assertEqual([(r[0], r[1], r[3], r[4]) for r in identities(refs)],
                             [(r[0], r[1], r[3], r[4]) for r in old_refs])
            (root / "crates/kdr-linter/src/lib.rs").write_text('pub fn value() -> u32 { 9 }\n')
            with self.assertRaisesRegex(RuntimeError, "source inputs changed"):
                cache.verify()
            rebuilt = [r for r in cargo_messages(real_cargo(root, build, cargo_env(target)))
                       if r.get("reason") == "compiler-artifact"]
            self.assertTrue(any(r["fresh"] is False for r in rebuilt), rebuilt)
            executed = subprocess.run([str(profile / "mathjax_consumer_facts")], cwd=root,
                                      text=True, capture_output=True, check=False, timeout=10)
            self.assertEqual((executed.returncode, executed.stdout.strip()), (0, "9"))


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
