#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from functools import partial
from unittest.mock import patch

import cargo_capacity_guard_native_cli_observation as observation
from cargo_capacity_guard import JsonObject, _array, _object, _text, inspect


class Fixture:
    def __init__(self, directory: Path, action: str = "") -> None:
        directory = directory.resolve()
        self.root = directory / "repository"
        self.root.mkdir()
        self.tools = directory / "tools"
        self.tools.mkdir()
        self.home = directory / "home"
        self.home.mkdir()
        self.nonce = "a" * 64
        self.attempt = self.root / "tmp" / self.nonce
        self.attempt.mkdir(parents=True)
        self.target = self.root / "target"
        self.binary = self.target / "debug/krr"
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b"before")
        self.request_path = self.attempt / "request.json"
        self.cargo = self.tools / "cargo"
        rustc = self.tools / "rustc"
        rustc.write_text("#!/usr/bin/false\n")
        rustc.chmod(0o700)
        child = (
            "import json,os,pathlib,signal,subprocess,sys,time\n"
            f"root=pathlib.Path({str(self.root)!r}); attempt=pathlib.Path({str(self.attempt)!r})\n"
            "(attempt/'child.json').write_text(json.dumps({'argv':sys.argv,'environment':dict(os.environ),"
            "'pid':os.getpid(),'parent':os.getppid(),'sid':os.getsid(0),'pgid':os.getpgrp()}))\n"
            + action + "\n"
            "binary=root/'target/debug/krr'; binary.write_bytes(b'after')\n"
            "event={'reason':'compiler-artifact','package_id':'cli-id','target':{'name':'krr','kind':['bin'],"
            "'src_path':str(root/'crates/katana-render-runtime-cli/src/main.rs')},"
            "'executable':str(binary),'filenames':[str(binary)],'fresh':False,'features':[],"
            "'profile':{'opt_level':'0','debuginfo':2,'debug_assertions':True,'overflow_checks':True,'test':False}}\n"
            "print(json.dumps(event),flush=True); print(json.dumps({'reason':'build-finished','success':True}),flush=True)\n"
        )
        self.cargo.write_text("#!" + str(Path(sys.executable).resolve()) + "\n" + child)
        self.cargo.chmod(0o700)
        self.command = [str(self.cargo), *observation.BUILD_FLAGS]
        self.protected = self.root / "tmp/protected.bin"
        self.protected.write_bytes(b"protected")
        self.environment = {
            "PATH": str(self.tools) + os.pathsep + os.defpath, "HOME": str(self.home),
            "RUSTFLAGS": "-D warnings", "LC_CTYPE": "UTF-8", "CARGO_TARGET_DIR": str(self.target),
            "CARGO_CAPACITY_PROTECTED_REFS": json.dumps([str(self.protected)]),
        }
        self.environment.update({key: "" for key in observation.EMPTY_JUST_ENVIRONMENT_KEYS})
        self.environment.update(PWD=str(self.root), SHLVL="0", _=shutil.which("python3", path=self.environment["PATH"]) or "")
        if sys.platform == "darwin":
            self.environment["__CF_USER_TEXT_ENCODING"] = os.environ.get("__CF_USER_TEXT_ENCODING", f"0x{os.getuid():X}:0x0:0x0")
        self.root.joinpath(".gitignore").write_text("tmp/\ntarget/\n")
        source_paths = ("Cargo.toml", "Cargo.lock", "Justfile", "crates/katana-render-runtime-cli/Cargo.toml",
                        "crates/katana-render-runtime-cli/src/main.rs")
        for name in source_paths:
            destination = self.root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(name)
        self.root.joinpath("crates/katana-render-runtime-cli/Cargo.toml").write_text(
            '[package]\nname = "katana-render-runtime-cli"\nversion = "0.4.24"\n')
        managed = Path(__file__).parent
        for name in ("cargo_capacity_guard.py", "cargo_capacity_guard_lock.py", observation.__file__.split("/")[-1]):
            destination = self.root / "scripts/build" / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(managed / name, destination)
        subprocess.run(["git", "init", "--quiet", str(self.root)], check=True, capture_output=True)
        rows = sorted((observation.file_record(path) for path in self.root.rglob("*")
                       if path.is_file() and not path.is_relative_to(self.root / ".git")
                       and not path.is_relative_to(self.root / "tmp") and not path.is_relative_to(self.target)),
                      key=lambda row: _text(row["path"], "path").encode())
        self.source = self.attempt / "source.json"
        self.write_json(self.source, {"schema": "krr-native-build-source-manifest-v1",
                                     "files": rows, "digest": observation.source_digest(rows)})
        self.state = self.root / "tmp/.cargo-capacity-guard.json"
        self.write_json(self.state, {key: value for key, value in observation.policy(self.target).items()
                                     if key != "threshold_bytes"})
        metadata = self.attempt / "metadata.json"
        self.write_json(metadata, {"version": 1, "workspace_root": str(self.root), "target_directory": str(self.target),
            "packages": [{"id": "cli-id", "name": "katana-render-runtime-cli", "features": {},
                          "manifest_path": str(self.root / "crates/katana-render-runtime-cli/Cargo.toml"),
                          "targets": [{"name": "krr", "kind": ["bin"],
                                       "src_path": str(self.root / "crates/katana-render-runtime-cli/src/main.rs")}]}]})
        stderr = self.attempt / "metadata.stderr.raw"
        stderr.write_bytes(b"")
        probe = self.attempt / "metadata-observation.json"
        rtk = shutil.which("rtk")
        if rtk is None:
            raise AssertionError("RTK fixture provenance unavailable")
        self.write_json(probe, {"argv": [str(Path(rtk).resolve()), "proxy", str(self.cargo), "metadata", "--frozen",
                                       "--format-version", "1", "--no-deps", "--manifest-path", str(self.root / "Cargo.toml")],
            "stdout": observation.file_record(metadata), "stderr": observation.file_record(stderr),
            "label": "probe-1-cargo-metadata", "cwd": str(self.root), "attemptToken": self.nonce,
            "environment": self.environment, "environmentSha256": observation.environment_digest(self.environment),
            "pid": os.getpid(), "actualExit": 0, "startedNs": "1", "endedNs": "2", "reaped": True, "errors": []})
        pin = observation.file_record(self.protected)
        value = self.protected.stat()
        self.protected_row = {**pin, "dev": value.st_dev, "ino": value.st_ino, "mode": value.st_mode}
        self.registry = self.root / "tmp/registry.json"
        self.write_json(self.registry, {"schema": "krr-protected-refs-successor-v1", "physicalRefCount": 1,
                                       "refs": [self.protected_row]})
        issued = time.monotonic_ns()
        self.request: JsonObject = {
            "schema": observation.REQUEST_SCHEMA, "attemptToken": self.nonce, "cwd": str(self.root),
            "target": str(self.target), "binaryPath": str(self.binary),
            "issuedMonotonicNs": str(issued), "expiresMonotonicNs": str(issued + observation.REQUEST_TTL_NS),
            "cargo": observation.file_record(self.cargo), "sourceManifest": observation.file_record(self.source),
            "sourceDigest": observation.source_digest(rows), "metadata": observation.file_record(metadata),
            "metadataObservation": observation.file_record(probe),
            "expectedPackage": {"id": "cli-id", "name": "katana-render-runtime-cli",
                                "manifestPath": str(self.root / "crates/katana-render-runtime-cli/Cargo.toml")},
            "expectedTarget": {"name": "krr", "kind": ["bin"],
                               "srcPath": str(self.root / "crates/katana-render-runtime-cli/src/main.rs")},
            "environment": self.environment, "environmentSha256": observation.environment_digest(self.environment),
            "guardCodePins": [observation.file_record(self.root / "scripts/build" / name) for name in
                              ("cargo_capacity_guard.py", "cargo_capacity_guard_lock.py", Path(observation.__file__).name)],
            "capacityAdmission": {"state": observation.file_record(self.state),
                "protectedRegistry": observation.file_record(self.registry), "refs": [self.protected_row],
                "capacityPolicy": observation.policy(self.target),
                "environmentValue": self.environment["CARGO_CAPACITY_PROTECTED_REFS"],
                "environmentSha256": hashlib.sha256(self.environment["CARGO_CAPACITY_PROTECTED_REFS"].encode()).hexdigest()},
        }
        self.save_request()

    @staticmethod
    def write_json(path: Path, value: object) -> None:
        path.write_text(json.dumps(value))

    def save_request(self) -> None:
        self.write_json(self.request_path, self.request)
        self.full_environment = {**self.environment, observation.REQUEST_ENV: str(self.request_path),
                                 observation.REQUEST_SHA_ENV: hashlib.sha256(self.request_path.read_bytes()).hexdigest()}

    def save_environment_proof(self) -> None:
        self.request["environmentSha256"] = observation.environment_digest(self.environment)
        probe_path = Path(_text(_object(self.request["metadataObservation"], "probe")["path"], "probe path"))
        probe = observation.parse_json(probe_path.read_bytes())
        probe["environment"] = self.environment
        probe["environmentSha256"] = self.request["environmentSha256"]
        self.write_json(probe_path, probe)
        self.request["metadataObservation"] = observation.file_record(probe_path)
        self.save_request()

    def admission(self) -> observation.Admission:
        with patch.dict(os.environ, self.full_environment, clear=True):
            return observation.validate_request(self.root, self.command, os.environ)

    def run(self) -> tuple[int, JsonObject]:
        with patch.dict(os.environ, self.full_environment, clear=True):
            admission = observation.validate_request(self.root, self.command, os.environ)
            report = inspect(self.root, self.target, self.state, 2, admission.state_sha256)
            code = observation.observe(admission, self.command, self.root, report,
                                       lambda: inspect(self.root, self.target, self.state, 2, admission.state_sha256))
        return code, observation.parse_json((self.attempt / "observation.json").read_bytes())

    def events(self) -> list[JsonObject]:
        return [{"reason": "compiler-artifact", "package_id": "cli-id", "target": {"name": "krr", "kind": ["bin"],
                 "src_path": str(self.root / "crates/katana-render-runtime-cli/src/main.rs")},
                 "executable": str(self.binary), "filenames": [str(self.binary)], "fresh": False,
                 "features": [], "profile": {"opt_level": "0", "debuginfo": 2, "debug_assertions": True,
                                               "overflow_checks": True, "test": False}},
                {"reason": "build-finished", "success": True}]


class NativeObservationTests(unittest.TestCase):
    def test_controlled_process_records_actual_pid_argv_environment_and_raw(self) -> None:
        for fresh in (True, False):
            with self.subTest(fresh=fresh), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory))
                if fresh:
                    fixture.cargo.write_text(fixture.cargo.read_text().replace("'fresh':False", "'fresh':True"))
                    fixture.request["cargo"] = observation.file_record(fixture.cargo)
                    fixture.save_request()
                code, output = fixture.run()
                self.assertEqual(code, 0, output["errors"])
                self.assertEqual(output["status"], "cargo-completed")
                self.assertIs(output["accepted"], False)
                self.assertIs(output["externalGroupClosureRequired"], True)
                self.assertNotIn("groupClosed", output)
                owned = _object(output["ownedCargo"], "owned")
                child = observation.parse_json((fixture.attempt / "child.json").read_bytes())
                self.assertEqual(owned["pid"], child["pid"])
                self.assertEqual(owned["parentPid"], child["parent"])
                self.assertEqual(owned["sessionId"], child["sid"])
                self.assertEqual(owned["processGroupId"], child["pgid"])
                self.assertEqual(owned["argv"], [*fixture.command, observation.JSON_FLAG])
                child_environment = _object(child["environment"], "child environment")
                # macOSのPython fixture自身が追加するCoreFoundation値だけはCargoへの渡し値と区別する。
                extras = set(child_environment) - set(fixture.environment)
                self.assertEqual(extras, set())
                self.assertEqual(owned["environment"], {key: child_environment[key] for key in fixture.environment})
                self.assertNotIn(observation.REQUEST_ENV, _object(child["environment"], "env"))
                self.assertEqual(owned["actualExit"], 0)
                self.assertIs(owned["reaped"], True)
                self.assertIs(owned["rawFdsClosed"], True)
                for key, filename in (("stdout", "cargo.stdout.raw"), ("stderr", "cargo.stderr.raw")):
                    self.assertEqual(owned[key], observation.file_record(fixture.attempt / filename))
                self.assertEqual(output["pinsBefore"], output["pinsAfter"])
                with self.assertRaisesRegex(RuntimeError, "already consumed"):
                    fixture.admission()

    def test_request_shape_and_provenance_rejections_spawn_nothing(self) -> None:
        variants: list[tuple[str, object]] = [
            ("schema", "krr-native-cargo-observer-request-v1"), ("schema", "krr-native-cargo-observer-request-v2"),
            ("attemptToken", "b" * 64), ("cwd", "relative"), ("binaryPath", "/outside/krr"),
            ("target", "/outside/target"), ("sourceDigest", "b" * 64), ("environmentSha256", "b" * 64),
            ("issuedMonotonicNs", "0"), ("issuedMonotonicNs", True),
            ("expectedPackage", {"id": "other", "name": "katana-render-runtime-cli", "manifestPath": "/outside"}),
            ("expectedTarget", {"name": "krr", "kind": ["lib"], "srcPath": "/outside"}),
            ("unknown", 1),
        ]
        for key, value in variants:
            with self.subTest(key=key, value=value), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory))
                fixture.request[key] = value
                fixture.save_request()
                with self.assertRaises((RuntimeError, OSError)):
                    fixture.admission()
                self.assertFalse((fixture.attempt / "child.json").exists())

    def test_native_admission_uses_explicit_child_path_and_keeps_direct_cargo_binding(self) -> None:
        for variation in ("parent-proxy-child-canonical", "wrong-child-path", "different-cargo", "bare-name", "wrapper"):
            with self.subTest(variation=variation), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory))
                parent_tools = fixture.tools.parent / "parent-tools"
                parent_tools.mkdir()
                parent_cargo = parent_tools / "cargo"
                parent_cargo.write_text("#!/usr/bin/false\n")
                parent_cargo.chmod(0o700)
                parent_environment = {"PATH": str(parent_tools) + os.pathsep + os.defpath,
                                      "HOME": str(fixture.home)}
                if variation == "wrong-child-path":
                    fixture.environment["PATH"] = parent_environment["PATH"]
                    fixture.save_environment_proof()
                elif variation == "different-cargo":
                    fixture.command[0] = str(parent_cargo)
                elif variation == "bare-name":
                    fixture.command[0] = "cargo"
                elif variation == "wrapper":
                    fixture.command = [str(Path(shutil.which("rtk") or "").resolve()), "proxy", *fixture.command]
                with patch.dict(os.environ, parent_environment, clear=True):
                    if variation == "parent-proxy-child-canonical":
                        admission = observation.validate_request(fixture.root, fixture.command, fixture.full_environment)
                        self.assertEqual(admission.environment, fixture.environment)
                        self.assertNotEqual(Path(parent_cargo).resolve(), fixture.cargo.resolve())
                    else:
                        with self.assertRaises(RuntimeError):
                            observation.validate_request(fixture.root, fixture.command, fixture.full_environment)
                self.assertFalse((fixture.attempt / "child.json").exists())

    def test_request_ttl_pair_unknown_environment_and_wrapper_rejections(self) -> None:
        for variation in ("expired", "future", "missing-sha", "missing-path", "wrapper", "extra-flag", "rustc", "config"):
            with self.subTest(variation=variation), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory))
                if variation in ("expired", "future"):
                    issued = time.monotonic_ns() + (1_000_000_000 if variation == "future" else -121_000_000_000)
                    fixture.request["issuedMonotonicNs"] = str(issued)
                    fixture.request["expiresMonotonicNs"] = str(issued + observation.REQUEST_TTL_NS)
                    fixture.save_request()
                elif variation == "missing-sha":
                    fixture.full_environment.pop(observation.REQUEST_SHA_ENV)
                elif variation == "missing-path":
                    fixture.full_environment.pop(observation.REQUEST_ENV)
                elif variation == "wrapper":
                    fixture.command = [str(Path(shutil.which("rtk") or "").resolve()), "proxy", *fixture.command]
                elif variation == "extra-flag":
                    fixture.command.append("--release")
                elif variation == "rustc":
                    fixture.full_environment["RUSTC_WRAPPER"] = "/arbitrary"
                else:
                    config = fixture.home / ".cargo/config.toml"
                    config.parent.mkdir(parents=True)
                    config.write_text('[build]\nrustc-wrapper = "arbitrary"\n')
                with self.assertRaises((RuntimeError, OSError)):
                    fixture.admission()
                self.assertFalse((fixture.attempt / "child.json").exists())

    def test_current_registry_role_order_and_protected_hardlink_aliases_are_preserved(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory))
            alias = fixture.root / "tmp/aaa-protected-alias.bin"
            os.link(fixture.protected, alias)
            row = {**fixture.protected_row, "path": str(alias)}
            refs = [fixture.protected_row, row]
            fixture.write_json(fixture.registry, {"schema": "krr-protected-refs-successor-v1",
                                                  "physicalRefCount": 2, "refs": refs})
            cap = _object(fixture.request["capacityAdmission"], "capacity")
            cap["refs"] = refs
            cap["protectedRegistry"] = observation.file_record(fixture.registry)
            value = json.dumps([str(fixture.protected), str(alias)])
            fixture.environment["CARGO_CAPACITY_PROTECTED_REFS"] = value
            cap["environmentValue"] = value
            cap["environmentSha256"] = hashlib.sha256(value.encode()).hexdigest()
            fixture.save_environment_proof()
            admission = fixture.admission()
            self.assertEqual(admission.protected, [str(fixture.protected), str(alias)])
            self.assertEqual(alias.stat().st_ino, fixture.protected.stat().st_ino)

    def test_existing_just_exports_are_admitted_only_when_empty(self) -> None:
        for key in observation.EMPTY_JUST_ENVIRONMENT_KEYS:
            with self.subTest(key=key), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory))
                fixture.admission()
                fixture.full_environment[key] = "selected"
                with self.assertRaisesRegex(RuntimeError, "selectors must be empty"):
                    fixture.admission()
                self.assertFalse((fixture.attempt / "child.json").exists())

    def test_just_shell_overlay_values_remain_exactly_bound(self) -> None:
        for key, value in (("PWD", "/wrong"), ("SHLVL", "1"), ("_", "/arbitrary/python3"),
                           ("__CF_USER_TEXT_ENCODING", "0x0:0x0:0x0")):
            with self.subTest(key=key), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory))
                fixture.full_environment[key] = value
                with self.assertRaises(RuntimeError):
                    fixture.admission()
                self.assertFalse((fixture.attempt / "child.json").exists())

    def test_duplicate_json_utf8_size_bool_symlink_and_role_alias(self) -> None:
        for variation in ("duplicate", "utf8", "size-bool", "symlink", "alias", "stale-output"):
            with self.subTest(variation=variation), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory))
                if variation == "duplicate":
                    fixture.request_path.write_bytes(b'{"schema":"x","schema":"y"}')
                elif variation == "utf8":
                    fixture.request_path.write_bytes(b"\xff")
                elif variation == "size-bool":
                    _object(fixture.request["cargo"], "cargo")["size"] = True
                    fixture.save_request()
                elif variation == "symlink":
                    alias = fixture.attempt / "alias.json"
                    alias.symlink_to(fixture.request_path)
                    fixture.full_environment[observation.REQUEST_ENV] = str(alias)
                elif variation == "alias":
                    fixture.request["metadataObservation"] = fixture.request["metadata"]
                    fixture.save_request()
                else:
                    (fixture.attempt / "observation.json").write_text("stale")
                if variation in ("duplicate", "utf8"):
                    fixture.full_environment[observation.REQUEST_SHA_ENV] = hashlib.sha256(fixture.request_path.read_bytes()).hexdigest()
                with self.assertRaises((RuntimeError, OSError)):
                    fixture.admission()

    def test_metadata_package_target_and_probe_negative_variants(self) -> None:
        for variation in ("name", "id", "manifest", "duplicate", "target-name", "kind", "source", "nonce", "exit", "environment"):
            with self.subTest(variation=variation), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory))
                pin = _object(fixture.request["metadata"], "metadata")
                path = Path(_text(pin["path"], "metadata path"))
                metadata = observation.parse_json(path.read_bytes())
                package = _object(_array(metadata["packages"], "packages")[0], "package")
                target = _object(_array(package["targets"], "targets")[0], "target")
                probe_path = Path(_text(_object(fixture.request["metadataObservation"], "probe")["path"], "probe path"))
                probe = observation.parse_json(probe_path.read_bytes())
                if variation in ("name", "id", "manifest"):
                    package[{"manifest": "manifest_path"}.get(variation, variation)] = "wrong"
                elif variation == "duplicate":
                    _array(metadata["packages"], "packages").append(dict(package))
                elif variation == "target-name":
                    target["name"] = "other"
                elif variation == "kind":
                    target["kind"] = ["lib"]
                elif variation == "source":
                    target["src_path"] = "/other"
                else:
                    probe[{"nonce": "attemptToken", "exit": "actualExit", "environment": "environment"}[variation]] = {
                        "nonce": "b" * 64, "exit": 1, "environment": {"PATH": "wrong"}}[variation]
                fixture.write_json(path, metadata)
                fixture.request["metadata"] = observation.file_record(path)
                probe["stdout"] = fixture.request["metadata"]
                fixture.write_json(probe_path, probe)
                fixture.request["metadataObservation"] = observation.file_record(probe_path)
                fixture.save_request()
                with self.assertRaises(RuntimeError):
                    fixture.admission()

    def test_cargo_events_reject_invalid_streams_from_valid_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory))
            admission = fixture.admission()
            self.assertEqual(observation.cargo_events(b"\n".join(json.dumps(event).encode() for event in fixture.events()), admission)[0]["fresh"], False)
            for variation in ("duplicate", "missing", "nonterminal", "failed", "fresh", "kind", "src", "binary", "package", "json"):
                with self.subTest(variation=variation):
                    events = fixture.events()
                    if variation == "duplicate":
                        events.insert(0, dict(events[0]))
                    elif variation == "missing":
                        events.pop(0)
                    elif variation == "nonterminal":
                        events.append({"reason": "compiler-message"})
                    elif variation == "failed":
                        events[-1]["success"] = False
                    elif variation in ("kind", "src"):
                        _object(events[0]["target"], "target")[{"src": "src_path"}.get(variation, variation)] = "wrong"
                    elif variation != "json":
                        events[0][{"binary": "executable", "package": "package_id"}.get(variation, variation)] = 1
                    raw = b"\n".join(json.dumps(event).encode() for event in events)
                    if variation == "json":
                        raw += b"\nnot-json"
                    with self.assertRaises(RuntimeError):
                        observation.cargo_events(raw, admission)

    def test_review_feature_and_profile_counterexamples_reject_from_valid_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory))
            admission = fixture.admission()
            self.assertEqual(admission.expected_features, ())
            baseline = fixture.events()
            observation.cargo_events(b"\n".join(json.dumps(event).encode() for event in baseline), admission)
            for variation in ("missing-features", "unauthorized-features", "boolean-features",
                              "missing-profile", "boolean-profile", "test-profile"):
                with self.subTest(variation=variation):
                    events = fixture.events()
                    artifact = events[0]
                    if variation.startswith("missing-"):
                        artifact.pop(variation.removeprefix("missing-"))
                    elif variation == "unauthorized-features":
                        artifact["features"] = ["unauthorized"]
                    elif variation.startswith("boolean-"):
                        artifact[variation.removeprefix("boolean-")] = True
                    else:
                        _object(artifact["profile"], "profile")["test"] = True
                    with self.assertRaises(RuntimeError):
                        observation.cargo_events(b"\n".join(json.dumps(event).encode() for event in events), admission)

    def test_cargo_profile_documented_types_and_non_test_are_strict(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory))
            for debug in (None, 0, 1, 2, "line-directives-only", "line-tables-only"):
                with self.subTest(debug=debug):
                    artifact = fixture.events()[0]
                    _object(artifact["profile"], "profile")["debuginfo"] = debug
                    observation.validate_cli_artifact_configuration(artifact, ())
            for field, value in (("opt_level", True), ("opt_level", 0), ("opt_level", "invalid"),
                                 ("debuginfo", True), ("debuginfo", 3), ("debuginfo", "full"),
                                 ("debug_assertions", 1), ("overflow_checks", "true"), ("test", 0),
                                 ("unknown", True)):
                with self.subTest(field=field, value=value):
                    artifact = fixture.events()[0]
                    _object(artifact["profile"], "profile")[field] = value
                    with self.assertRaises(RuntimeError):
                        observation.validate_cli_artifact_configuration(artifact, ())
            for field in ("opt_level", "debuginfo", "debug_assertions", "overflow_checks", "test"):
                with self.subTest(missing=field):
                    artifact = fixture.events()[0]
                    _object(artifact["profile"], "profile").pop(field)
                    with self.assertRaises(RuntimeError):
                        observation.validate_cli_artifact_configuration(artifact, ())
            for features in (["unexpected"], ["repeated", "repeated"], [True], [1]):
                with self.subTest(features=features):
                    artifact = fixture.events()[0]
                    artifact["features"] = features
                    with self.assertRaises(RuntimeError):
                        observation.validate_cli_artifact_configuration(artifact, ())

    def test_current_cli_metadata_feature_map_is_required_and_empty(self) -> None:
        for variation in ("missing", "boolean", "unauthorized"):
            with self.subTest(variation=variation), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory))
                fixture.admission()
                path = Path(_text(_object(fixture.request["metadata"], "metadata")["path"], "metadata path"))
                metadata = observation.parse_json(path.read_bytes())
                package = _object(_array(metadata["packages"], "packages")[0], "package")
                if variation == "missing":
                    package.pop("features")
                else:
                    package["features"] = True if variation == "boolean" else {"unauthorized": []}
                fixture.write_json(path, metadata)
                fixture.request["metadata"] = observation.file_record(path)
                probe_path = Path(_text(_object(fixture.request["metadataObservation"], "probe")["path"], "probe path"))
                probe = observation.parse_json(probe_path.read_bytes())
                probe["stdout"] = fixture.request["metadata"]
                fixture.write_json(probe_path, probe)
                fixture.request["metadataObservation"] = observation.file_record(probe_path)
                fixture.save_request()
                with self.assertRaises(RuntimeError):
                    fixture.admission()
                self.assertFalse((fixture.attempt / "child.json").exists())

    def test_current_cli_manifest_feature_map_must_match_no_feature_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory))
            self.assertEqual(observation.validate_metadata(fixture.request, fixture.root, fixture.attempt), ())
            manifest = fixture.root / "crates/katana-render-runtime-cli/Cargo.toml"
            manifest.write_text(manifest.read_text() + "\n[features]\nunauthorized = []\n")
            with self.assertRaisesRegex(RuntimeError, "current CLI must have no package features"):
                observation.validate_metadata(fixture.request, fixture.root, fixture.attempt)
            self.assertFalse((fixture.attempt / "child.json").exists())

    def test_source_tool_code_capacity_and_protected_mutations_before_spawn(self) -> None:
        for variation in ("source", "new-source", "tool", "code", "state", "registry", "protected", "policy-bool"):
            with self.subTest(variation=variation), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory))
                admission = fixture.admission()
                report = inspect(fixture.root, fixture.target, fixture.state, 2, admission.state_sha256)
                if variation == "new-source":
                    (fixture.root / "extra.rs").write_text("new")
                elif variation == "policy-bool":
                    cap = _object(fixture.request["capacityAdmission"], "capacity")
                    _object(cap["capacityPolicy"], "policy")["metric_version"] = True
                    fixture.save_request()
                else:
                    path = {"source": fixture.root / "Cargo.lock", "tool": fixture.cargo,
                            "code": fixture.root / "scripts/build/cargo_capacity_guard.py", "state": fixture.state,
                            "registry": fixture.registry, "protected": fixture.protected}[variation]
                    path.write_bytes(path.read_bytes() + b"changed")
                with patch.dict(os.environ, fixture.full_environment, clear=True):
                    code = observation.observe(admission, fixture.command, fixture.root, report, partial(dict, report))
                self.assertEqual(code, 1)
                self.assertFalse((fixture.attempt / "child.json").exists())

    def test_real_child_early_exit_timeout_interrupt_and_primary_cleanup_failures(self) -> None:
        actions = {"early": "sys.exit(7)", "timeout": "time.sleep(60)",
                   "interrupt": "os.kill(os.getppid(),signal.SIGINT); time.sleep(60)",
                   "cleanup": "signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(60)"}
        for variation, action in actions.items():
            with self.subTest(variation=variation), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory), action)
                with patch.object(observation, "BUILD_LIMIT_SECONDS", 2), patch.object(observation, "TERMINATE_LIMIT_SECONDS", 0.05):
                    code, output = fixture.run()
                self.assertEqual(code, 1)
                self.assertEqual(output["status"], "failed")
                owned = _object(output["ownedCargo"], "owned")
                self.assertIs(owned["reaped"], True)
                self.assertIs(owned["rawFdsClosed"], True)
                self.assertTrue(output["errors"])
                if variation == "early":
                    self.assertEqual(owned["actualExit"], 7)
                if variation == "cleanup":
                    self.assertEqual(owned["signal"], signal.SIGKILL)
                    self.assertGreaterEqual(len(_array(output["errors"], "errors")), 2)

    def test_raw_fd_and_observation_exclusive_failure(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory))
            admission = fixture.admission()
            (fixture.attempt / "cargo.stderr.raw").write_bytes(b"preexisting")
            with patch.dict(os.environ, fixture.full_environment, clear=True):
                report = inspect(fixture.root, fixture.target, fixture.state, 2, admission.state_sha256)
                code = observation.observe(admission, fixture.command, fixture.root, report, partial(dict, report))
            output = observation.parse_json((fixture.attempt / "observation.json").read_bytes())
            self.assertEqual(code, 1)
            self.assertFalse((fixture.attempt / "child.json").exists())
            self.assertIs(_object(output["ownedCargo"], "owned")["rawFdsClosed"], False)
            self.assertEqual((fixture.attempt / "cargo.stderr.raw").read_bytes(), b"preexisting")
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory), "(attempt/'observation.json').write_text('occupied')")
            with self.assertRaisesRegex(RuntimeError, "observation save failed; prior errors"):
                fixture.run()
            self.assertEqual((fixture.attempt / "observation.json").read_text(), "occupied")

    def test_real_spawn_failure_and_pre_capacity_failure_keep_cargo_zero(self) -> None:
        for variation in ("spawn", "metric", "capacity"):
            with self.subTest(variation=variation), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory))
                if variation == "spawn":
                    fixture.cargo.write_text("this is not an executable image")
                    fixture.request["cargo"] = observation.file_record(fixture.cargo)
                    fixture.save_request()
                with patch.dict(os.environ, fixture.full_environment, clear=True):
                    admission = observation.validate_request(fixture.root, fixture.command, os.environ)
                    report = inspect(fixture.root, fixture.target, fixture.state, 2, admission.state_sha256)
                    if variation == "metric":
                        report["metric_available"] = False
                    elif variation == "capacity":
                        report["current_bytes"] = observation.THRESHOLD + 1
                        report["over_capacity"] = True
                    code = observation.observe(admission, fixture.command, fixture.root, report, partial(dict, report))
                output = observation.parse_json((fixture.attempt / "observation.json").read_bytes())
                self.assertEqual(code, 1)
                self.assertIsNone(_object(output["ownedCargo"], "owned")["pid"])
                self.assertFalse((fixture.attempt / "child.json").exists())
                self.assertTrue(output["errors"])

    def test_post_rustc_config_and_raw_inode_replacement_fail(self) -> None:
        for variation in ("rustc", "config", "raw"):
            with self.subTest(variation=variation), tempfile.TemporaryDirectory() as directory:
                if variation == "rustc":
                    action = "pathlib.Path(os.environ['PATH'].split(os.pathsep)[0]).joinpath('rustc').write_text('changed')"
                elif variation == "config":
                    action = "p=pathlib.Path(os.environ['HOME'])/'.cargo'; p.mkdir(); (p/'config.toml').write_text('[build]')"
                else:
                    action = "p=attempt/'cargo.stdout.raw'; p.rename(attempt/'original.stdout.raw'); p.write_text('replacement')"
                fixture = Fixture(Path(directory), action)
                code, output = fixture.run()
                self.assertEqual(code, 1)
                self.assertEqual(output["status"], "failed")
                self.assertTrue(output["errors"])

    def test_post_source_protection_and_capacity_failures(self) -> None:
        actions = {"source": "(root/'Cargo.lock').write_text('changed')",
                   "protection": "(root/'tmp/protected.bin').write_text('changed')",
                   "capacity": ""}
        for variation, action in actions.items():
            with self.subTest(variation=variation), tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory), action)
                with patch.dict(os.environ, fixture.full_environment, clear=True):
                    admission = observation.validate_request(fixture.root, fixture.command, os.environ)
                    report = inspect(fixture.root, fixture.target, fixture.state, 2, admission.state_sha256)
                    post = dict(report)
                    if variation == "capacity":
                        post["current_bytes"] = observation.THRESHOLD + 1
                        post["over_capacity"] = True
                    code = observation.observe(admission, fixture.command, fixture.root, report, partial(dict, post))
                self.assertEqual(code, 1)
                output = observation.parse_json((fixture.attempt / "observation.json").read_bytes())
                self.assertEqual(output["status"], "failed")
                self.assertTrue(output["errors"])

    def test_descendant_does_not_claim_group_closure(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory), "child=subprocess.Popen([sys.executable,'-c','import time;time.sleep(60)']); (attempt/'descendant.pid').write_text(str(child.pid))")
            descendant: int | None = None
            try:
                code, output = fixture.run()
                descendant = int((fixture.attempt / "descendant.pid").read_text())
                os.kill(descendant, 0)
                self.assertEqual(code, 0, output["errors"])
                self.assertIs(output["externalGroupClosureRequired"], True)
                self.assertNotIn("groupClosed", output)
                self.assertNotIn("groupClosed", _object(output["ownedCargo"], "owned"))
            finally:
                if descendant is not None:
                    os.kill(descendant, signal.SIGKILL)

    def test_capacity_policy_never_accepts_bool_or_unavailable_or_excess(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory))
            report = inspect(fixture.root, fixture.target, fixture.state, 2)
            observation.validate_capacity_report(report, fixture.target)
            for key, value in (("metric_version", True), ("baseline_bytes", None), ("current_bytes", True),
                               ("metric_available", False), ("over_capacity", True), ("factor", 3),
                               ("cleanup_status", "completed"), ("removed", ["something"])):
                with self.subTest(key=key):
                    changed = dict(report)
                    changed[key] = value
                    with self.assertRaises(RuntimeError):
                        observation.validate_capacity_report(changed, fixture.target)


if __name__ == "__main__":
    unittest.main()
