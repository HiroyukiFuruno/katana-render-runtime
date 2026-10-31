"""Run the scoped KRR runtime-script Moon CAS diagnostic under exclusive ownership."""

from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import time
from collections.abc import Mapping
from pathlib import Path
from typing import Literal

from moon_cache_contract_test import (
    MOON_TASK_CONTEXT_VARIABLES,
    ROOT,
    RUNTIME_UNUSED_LEAF_IDS,
    digest,
    moon_json_object,
)

ASSESSMENT = ROOT / "tmp/quality/runtime-script-moon-cas"

RUNTIME_CAS_PRODUCER = "runtime-unused-fragment-loader"
RUNTIME_CAS_TASK = "krr:runtime-unused-fragment-loader-producer"
RUNTIME_CAS_DIAGNOSTIC_TARGET = "krr:runtime-script-cas-diagnostic"
RUNTIME_CAS_OPERATION_ENV = "KRR_RUNTIME_CAS_OPERATION"
RUNTIME_CAS_FIXED_OPERATIONS = ("validate-all", "ensure-all", "host-verify")
RUNTIME_CAS_RECEIPT = (
    "tmp/quality/runtime-script-results/runtime-unused-fragment-loader.json"
)
RUNTIME_CAS_JUNIT = (
    "tmp/quality/runtime-script-results/"
    "runtime-unused-fragment-loader-current-group-001.xml"
)
RUNTIME_CAS_OTHER_RECEIPTS = tuple(
    f"tmp/quality/runtime-script-results/{producer}.json"
    for producer in RUNTIME_UNUSED_LEAF_IDS
    if producer != RUNTIME_CAS_PRODUCER
)
RUNTIME_CAS_OTHER_JUNIT = tuple(
    f"tmp/quality/runtime-script-results/{producer}-current-group-001.xml"
    for producer in RUNTIME_UNUSED_LEAF_IDS
    if producer != RUNTIME_CAS_PRODUCER
)

def _runtime_cas_real_parent(path: Path) -> None:
    relative = path.parent.relative_to(ROOT)
    current = ROOT
    for component in (".", *relative.parts):
        if component != ".":
            current = current / component
        metadata = current.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise RuntimeError("runtime CAS path has a non-directory parent")


def _runtime_cas_safe_file(path: Path) -> bytes:
    _runtime_cas_real_parent(path)
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        raise RuntimeError("runtime CAS diagnostic path is not a private regular file")
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(descriptor, "rb") as source:
        opened = os.fstat(source.fileno())
        if opened.st_ino != metadata.st_ino or opened.st_dev != metadata.st_dev:
            raise RuntimeError("runtime CAS diagnostic file changed while opening")
        return source.read()


def _runtime_cas_snapshot_files(
    paths: tuple[Path, ...],
) -> dict[Path, tuple[bytes, int]]:
    return {
        path: (_runtime_cas_safe_file(path), stat.S_IMODE(path.lstat().st_mode))
        for path in paths
    }


def _runtime_cas_files_match_snapshot(
    snapshot: Mapping[Path, tuple[bytes, int]],
) -> bool:
    return all(
        _runtime_cas_safe_file(path) == content
        and stat.S_IMODE(path.lstat().st_mode) == mode
        for path, (content, mode) in snapshot.items()
    )


def _runtime_cas_restore_file(path: Path, content: bytes, mode: int) -> None:
    _runtime_cas_real_parent(path)
    try:
        existing = path.lstat()
    except FileNotFoundError:
        existing = None
    if existing is not None and (
        stat.S_ISLNK(existing.st_mode)
        or not stat.S_ISREG(existing.st_mode)
        or existing.st_nlink != 1
    ):
        raise RuntimeError("runtime CAS restore target is not a private regular file")
    temporary = path.with_name(f".{path.name}.restore-{time.time_ns()}")
    descriptor = os.open(
        temporary,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        mode,
    )
    try:
        with os.fdopen(descriptor, "wb") as target:
            os.fchmod(target.fileno(), mode)
            target.write(content)
            target.flush()
            os.fsync(target.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def _runtime_cas_restore_entries(
    entries: Mapping[Path, tuple[bytes, int]],
) -> dict[str, str]:
    restored: dict[str, str] = {}
    errors: list[str] = []
    for path, (content, mode) in entries.items():
        try:
            _runtime_cas_restore_file(path, content, mode)
            if (
                _runtime_cas_safe_file(path) != content
                or stat.S_IMODE(path.lstat().st_mode) != mode
            ):
                raise RuntimeError("restored output differs from its saved bytes or mode")
            restored[str(path.relative_to(ROOT))] = hashlib.sha256(content).hexdigest()
        except Exception as error:
            errors.append(f"{path}: {error}")
    if errors:
        raise RuntimeError("runtime CAS restore errors: " + "; ".join(errors))
    return restored


def _runtime_cas_write_fd(descriptor: int, content: bytes) -> None:
    offset = 0
    while offset < len(content):
        offset += os.write(descriptor, content[offset:])
    os.fsync(descriptor)


RuntimeCasObserverRole = Literal["metadata", "build"]


def _runtime_cas_logged_process(
    run_dir: Path,
    label: str,
    command: list[str],
    environment: dict[str, str],
    *,
    role: RuntimeCasObserverRole,
    check: bool = True,
) -> tuple[int, Path, dict[str, object]]:
    release_directory = str(ROOT / "scripts/release")
    sys.path.insert(0, release_directory)
    try:
        from produce_html_performance_evidence import observe_native_cli_command
    finally:
        sys.path.remove(release_directory)
    token = hashlib.sha256(
        json.dumps([label, command], separators=(",", ":")).encode()
    ).hexdigest()
    evidence = observe_native_cli_command(
        command,
        role=role,
        log_directory=run_dir / "processes",
        cwd=ROOT,
        env=environment,
        attempt_token=token,
        label=label,
        check=check,
    )
    observation = evidence.observation
    stdout = moon_json_object(observation.get("stdout"))
    stderr = moon_json_object(observation.get("stderr"))
    stdout_path = Path(str(stdout["path"]))
    process_record: dict[str, object] = {
        "argv": observation.get("argv"),
        "actualExit": observation.get("actualExit"),
        "pid": observation.get("pid"),
        "stdout": stdout,
        "stderr": stderr,
        "lifecycle": evidence.lifecycle,
    }
    actual_exit = observation.get("actualExit")
    observed_argv = observation.get("argv")
    if type(actual_exit) is not int or observed_argv != command:
        raise RuntimeError("observed child argv or actual exit differs from invocation")
    return actual_exit, stdout_path, process_record


def _runtime_cas_child_environment(
    environment: Mapping[str, str] | None = None,
) -> dict[str, str]:
    source = os.environ if environment is None else environment
    if any(variable in source for variable in MOON_TASK_CONTEXT_VARIABLES):
        raise RuntimeError("runtime CAS orchestrator must run outside a Moon task")
    child_environment = dict(source)
    for variable in MOON_TASK_CONTEXT_VARIABLES:
        child_environment.pop(variable, None)
    child_environment["MOON_CACHE"] = "read-write"
    child_environment["MOON_CACHE_CAS_VERIFY_INTEGRITY"] = "true"
    child_environment["MOON_EXPERIMENT_CAS_OUTPUTS_CACHE"] = "true"
    return child_environment


def _runtime_cas_operation_spec(operation: str) -> tuple[str, str | None]:
    if operation in RUNTIME_CAS_FIXED_OPERATIONS:
        return operation, None
    prefix = "identity:"
    if operation.startswith(prefix):
        producer = operation.removeprefix(prefix)
        if producer in RUNTIME_UNUSED_LEAF_IDS:
            return "identity", producer
    raise RuntimeError(f"unknown runtime CAS operation: {operation}")


def _runtime_cas_operation_role(operation: str) -> RuntimeCasObserverRole:
    kind, _ = _runtime_cas_operation_spec(operation)
    return "build" if kind == "ensure-all" else "metadata"


def _runtime_cas_has_dedicated_task_environment(environment: Mapping[str, str]) -> bool:
    expected_task_id = RUNTIME_CAS_DIAGNOSTIC_TARGET.partition(":")[2]
    return (
        environment.get("MOON_TARGET") == RUNTIME_CAS_DIAGNOSTIC_TARGET
        and environment.get("MOON_TASK_ID") == expected_task_id
    )


def _runtime_cas_operation_result(path: Path, operation: str) -> dict[str, object]:
    kind, producer = _runtime_cas_operation_spec(operation)
    lines = path.read_bytes().decode("utf-8").splitlines()
    output_prefix = f"{RUNTIME_CAS_DIAGNOSTIC_TARGET} | "
    payloads: list[dict[str, object]] = []
    for line in lines:
        candidate = line.strip()
        if not candidate:
            continue
        if candidate.startswith(output_prefix):
            candidate = candidate.removeprefix(output_prefix).strip()
        elif "|" in candidate and candidate.split("|", 1)[1].lstrip().startswith(
            ('{', '[', '"')
        ):
            raise RuntimeError("runtime CAS operation emitted an unknown JSON prefix")
        elif candidate[0] not in '{["' and candidate not in {
            "true",
            "false",
            "null",
        }:
            continue
        if not candidate:
            raise RuntimeError("runtime CAS operation emitted an empty JSON payload")
        try:
            payload = json.loads(candidate)
        except json.JSONDecodeError as error:
            raise RuntimeError("runtime CAS operation emitted invalid JSON") from error
        if not isinstance(payload, dict):
            raise RuntimeError("runtime CAS operation emitted a non-object JSON value")
        payloads.append(payload)
    if kind == "host-verify":
        if payloads:
            raise RuntimeError("host verification emitted unexpected JSON")
        return {}
    if len(payloads) != 1:
        raise RuntimeError("runtime CAS operation did not emit exactly one JSON object")
    payload = payloads[0]
    if kind == "identity":
        if (
            payload.get("schema") != "krr-runtime-script-identity-v2"
            or payload.get("producer") != producer
        ):
            raise RuntimeError("runtime CAS identity payload schema is invalid")
    else:
        if (
            payload.get("schema") != "krr-runtime-script-validation-all-v1"
            or payload.get("valid") is not True
        ):
            raise RuntimeError("runtime CAS validation payload schema is invalid")
    return payload


def run_runtime_cas_operation(operation: str) -> int:
    kind, producer = _runtime_cas_operation_spec(operation)
    if not _runtime_cas_has_dedicated_task_environment(os.environ):
        raise RuntimeError("runtime CAS operation requires its dedicated Moon task")
    if kind == "identity":
        command = [
            sys.executable,
            str(ROOT / "scripts/release/runtime_script_identity.py"),
            "--repo",
            str(ROOT),
            "--producer",
            str(producer),
        ]
    elif kind in {"validate-all", "ensure-all"}:
        command = [
            sys.executable,
            str(ROOT / "scripts/release/runtime_script_cache_cli.py"),
            kind,
            "--repo",
            str(ROOT),
        ]
    else:
        command = [
            "bun",
            str(ROOT / "scripts/runtime-bundles/runtime-rust-facts-host.ts"),
            "verify",
            str(ROOT),
            str(ROOT / "tmp/quality/rust-facts-host/current/receipt.json"),
        ]
    return subprocess.run(command, cwd=ROOT, env=dict(os.environ), check=False).returncode


def _runtime_cas_action(cache_root: Path, run_dir: Path, label: str) -> dict[str, object]:
    report_path = cache_root / "runReport.json"
    report_bytes = _runtime_cas_safe_file(report_path)
    snapshot = run_dir / f"{label}.runReport.json"
    snapshot.write_bytes(report_bytes)
    report = moon_json_object(json.loads(report_bytes))
    actions = report.get("actions")
    if not isinstance(actions, list):
        raise RuntimeError("Moon run report has no action list")
    matches = []
    for raw_action in actions:
        action = moon_json_object(raw_action)
        node = moon_json_object(action.get("node"))
        params = moon_json_object(node.get("params"))
        if params.get("target") == RUNTIME_CAS_TASK:
            matches.append(action)
    if len(matches) != 1:
        raise RuntimeError("Moon report does not identify one target action")
    operations = matches[0].get("operations")
    if not isinstance(operations, list):
        raise RuntimeError("Moon target action has no operations")
    hashes = []
    for raw_operation in operations:
        meta = moon_json_object(moon_json_object(raw_operation).get("meta"))
        if meta.get("type") == "hash-generation":
            hashes.append(meta.get("hash"))
    if len(hashes) != 1 or not isinstance(hashes[0], str):
        raise RuntimeError("Moon target action has no unique action hash")
    return {
        "action": matches[0],
        "actionHash": hashes[0],
        "runReport": str(snapshot),
        "runReportSha256": hashlib.sha256(report_bytes).hexdigest(),
    }


def _runtime_cas_report_is_fresh(
    previous_report_sha256: str, action: Mapping[str, object]
) -> bool:
    current_report_sha256 = action.get("runReportSha256")
    return (
        isinstance(current_report_sha256, str)
        and current_report_sha256 != previous_report_sha256
    )


def _runtime_cas_manifest_blob(
    cache_root: Path, action_hash: str, relative_output: str
) -> tuple[Path, Path, bytes, dict[str, object]]:
    manifest_path = cache_root / "manifests" / action_hash[:2] / action_hash[2:]
    manifest = moon_json_object(json.loads(_runtime_cas_safe_file(manifest_path)))
    files = manifest.get("files")
    if not isinstance(files, list):
        raise RuntimeError("Moon output manifest has no file list")
    matches = [
        moon_json_object(item)
        for item in files
        if moon_json_object(item).get("path") == relative_output
    ]
    if len(matches) != 1:
        raise RuntimeError("Moon manifest does not pin one selected receipt")
    digest_record = moon_json_object(matches[0].get("digest"))
    blob_digest = digest_record.get("hash")
    size = digest_record.get("size")
    if (
        not isinstance(blob_digest, str)
        or re.fullmatch(r"[0-9a-f]{64}", blob_digest) is None
        or type(size) is not int
        or size < 1
    ):
        raise RuntimeError("Moon receipt blob identity is invalid")
    blob_path = cache_root / "blobs" / blob_digest[:2] / blob_digest[2:]
    blob_bytes = _runtime_cas_safe_file(blob_path)
    if len(blob_bytes) != size or hashlib.sha256(blob_bytes).hexdigest() != blob_digest:
        raise RuntimeError("Moon receipt blob differs from its manifest digest")
    return manifest_path, blob_path, blob_bytes, matches[0]


def _runtime_cas_unique_blob_owner(
    cache_root: Path, blob_digest: str, expected_manifest: Path, output_path: str
) -> int:
    owners: list[tuple[Path, str]] = []
    manifest_root = cache_root / "manifests"
    root_info = manifest_root.lstat()
    if stat.S_ISLNK(root_info.st_mode) or not stat.S_ISDIR(root_info.st_mode):
        raise RuntimeError("Moon manifest root is not a real directory")
    for current, directories, filenames in os.walk(manifest_root, followlinks=False):
        current_path = Path(current)
        for name in directories:
            info = (current_path / name).lstat()
            if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
                raise RuntimeError("Moon manifest inventory contains a non-directory")
        for filename in filenames:
            manifest_path = current_path / filename
            info = manifest_path.lstat()
            if stat.S_ISLNK(info.st_mode) or not stat.S_ISREG(info.st_mode):
                raise RuntimeError("Moon manifest inventory contains a non-file")
            entries = moon_json_object(json.loads(_runtime_cas_safe_file(manifest_path))).get("files")
            if not isinstance(entries, list):
                raise RuntimeError("Moon manifest inventory is ambiguous")
            for entry in entries:
                record = moon_json_object(entry)
                digest_record = moon_json_object(record.get("digest"))
                if digest_record.get("hash") == blob_digest:
                    owners.append((manifest_path, str(record.get("path"))))
    if owners != [(expected_manifest, output_path)]:
        raise RuntimeError("selected Moon CAS blob has more than one output owner")
    return len(owners)


def _runtime_cas_has_cached_hydration(operations: object) -> bool:
    if not isinstance(operations, list):
        return False
    for raw_operation in operations:
        operation = moon_json_object(raw_operation)
        meta = moon_json_object(operation.get("meta"))
        if meta.get("type") == "output-hydration" and operation.get("status") == "cached":
            return True
    return False


def _runtime_cas_raw_directories(raw_root: Path) -> set[str]:
    root_info = raw_root.lstat()
    if stat.S_ISLNK(root_info.st_mode) or not stat.S_ISDIR(root_info.st_mode):
        raise RuntimeError("runtime-script raw log root is not a real directory")
    names: set[str] = set()
    for path in raw_root.iterdir():
        info = path.lstat()
        if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
            raise RuntimeError("runtime-script raw log inventory is not directory-only")
        names.add(path.name)
    return names


def _runtime_cas_new_producer_log(
    raw_root: Path, previous: set[str], expected_command: list[str]
) -> dict[str, object]:
    current = _runtime_cas_raw_directories(raw_root)
    added = current - previous
    if len(added) != 1:
        raise RuntimeError("repair did not create one fresh producer log directory")
    directory = raw_root / next(iter(added))
    directory_info = directory.lstat()
    if stat.S_ISLNK(directory_info.st_mode) or not stat.S_ISDIR(directory_info.st_mode):
        raise RuntimeError("fresh producer log path is not a real directory")
    command_path = directory / "group-001-command.json"
    command = moon_json_object(json.loads(_runtime_cas_safe_file(command_path))).get("argv")
    if (
        not isinstance(command, list)
        or any(not isinstance(argument, str) for argument in command)
        or command[: len(expected_command)] != expected_command
        or command[len(expected_command) : len(expected_command) + 2]
        != ["--reporter=junit", "--reporter-outfile"]
        or len(command) != len(expected_command) + 3
    ):
        raise RuntimeError("fresh producer command does not match the fixed profile")
    output_path = command[-1]
    if (
        not isinstance(output_path, str)
        or not output_path.startswith(
            f"tmp/quality/runtime-script-results/{RUNTIME_CAS_PRODUCER}-"
        )
        or not output_path.endswith("-group-001.xml")
    ):
        raise RuntimeError("fresh producer JUnit output is outside its fixed stage")
    exit_path = directory / "group-001-exit.txt"
    if _runtime_cas_safe_file(exit_path) != b"0\n":
        raise RuntimeError("fresh producer did not exit successfully")
    raw_files = ["group-001-command.json", "group-001-exit.txt", "group-001-stdout.log", "group-001-stderr.log"]
    pins = {name: hashlib.sha256(_runtime_cas_safe_file(directory / name)).hexdigest() for name in raw_files}
    return {"directory": str(directory), "command": command, "filesSha256": pins}


def _runtime_cas_controlled_failed_closed(
    exit_code: int,
    outputs_absent: bool,
    same_action: bool,
    producer_logs_unchanged: bool,
) -> bool:
    return exit_code != 0 and all(
        (outputs_absent, same_action, producer_logs_unchanged)
    )


def _runtime_cas_outputs_absent(outputs: tuple[Path, ...]) -> bool:
    return all(not path.exists() and not path.is_symlink() for path in outputs)


def _runtime_cas_probe_acceptance(
    result: Mapping[str, object],
    outputs_absent: bool,
    producer_logs_unchanged: bool,
) -> str | None:
    exit_code = result.get("exit")
    same_action = result.get("sameAction") is True
    if type(exit_code) is int and _runtime_cas_controlled_failed_closed(
        exit_code, outputs_absent, same_action, producer_logs_unchanged
    ):
        return "controlled_failed_closed"
    if (
        result.get("classification") == "corrupt_blob_hydrated"
        and same_action
        and result.get("hydrationReported") is True
        and result.get("corruptBytesMatch") is True
        and producer_logs_unchanged
    ):
        return "corrupt_blob_hydrated"
    return None


def _runtime_cas_repair_input_mode(
    result: Mapping[str, object], outputs_absent: bool
) -> str:
    if result.get("classification") == "corrupt_blob_hydrated" and not outputs_absent:
        return "preserve-corrupt-output"
    if result.get("probeAcceptance") == "controlled_failed_closed" and outputs_absent:
        return "missing-output"
    return "unclassified"


def _runtime_cas_expected_command() -> list[str]:
    release_directory = str(ROOT / "scripts/release")
    sys.path.insert(0, release_directory)
    try:
        from runtime_script_recipe import read_runtime_script_recipe
    finally:
        sys.path.remove(release_directory)
    recipe = read_runtime_script_recipe(ROOT, RUNTIME_CAS_PRODUCER)
    if len(recipe.commands) != 1:
        raise RuntimeError("selected profile no longer has one declared Bun command")
    return list(recipe.commands[0])


class RuntimeScriptCasDiagnostic:
    """Probe one real KRR producer against Moon's local output CAS."""

    def __init__(self, moon_command: list[str], run_dir: Path) -> None:
        self.moon_command = moon_command
        self.run_dir = run_dir
        self.environment = _runtime_cas_child_environment()
        self.cache_root = ROOT / ".moon/cache"
        self.task = RUNTIME_CAS_TASK
        self.receipt = ROOT / RUNTIME_CAS_RECEIPT
        self.junit = ROOT / RUNTIME_CAS_JUNIT
        self.outputs = (self.receipt, self.junit)
        self.records: list[dict[str, object]] = []
        self.saved_files: dict[Path, tuple[bytes, int]] = {}
        self.seed_files: dict[Path, tuple[bytes, int]] = {}
        self.seed_report: dict[str, object] | None = None
        self.blob_path: Path | None = None
        self.blob_bytes: bytes | None = None
        self.blob_fd: int | None = None
        self.manifest_path: Path | None = None
        self.manifest_bytes: bytes | None = None
        self.manifest_mode: int | None = None
        self.blob_mode: int | None = None
        self.repaired_files: dict[Path, tuple[bytes, int]] = {}
        self.action_hash: str | None = None
        self.passed = False
        self.lease_path = (
            ROOT / "tmp/quality/runtime-script-results" / f".{RUNTIME_CAS_PRODUCER}.lease"
        )

    def _logged(
        self,
        label: str,
        command: list[str],
        *,
        role: RuntimeCasObserverRole,
        check: bool = True,
        environment: dict[str, str] | None = None,
    ) -> tuple[int, Path]:
        exit_code, stdout, process_record = _runtime_cas_logged_process(
            self.run_dir,
            label,
            command,
            self.environment if environment is None else environment,
            role=role,
            check=check,
        )
        self.records.append({"process": {"label": label, **process_record}})
        return exit_code, stdout

    def _moon_operation(
        self, label: str, operation: str, *, check: bool = True
    ) -> tuple[int, dict[str, object]]:
        _runtime_cas_operation_spec(operation)
        command = [
            *self.moon_command,
            "run",
            RUNTIME_CAS_DIAGNOSTIC_TARGET,
            "--upstream",
            "none",
            "--no-actions",
            "--summary",
            "detailed",
            "--color=false",
        ]
        operation_environment = dict(self.environment)
        operation_environment[RUNTIME_CAS_OPERATION_ENV] = operation
        exit_code, stdout = self._logged(
            label,
            command,
            check=check,
            environment=operation_environment,
            role=_runtime_cas_operation_role(operation),
        )
        if exit_code != 0:
            return exit_code, {}
        return exit_code, _runtime_cas_operation_result(stdout, operation)

    def _identity_set(self, label: str) -> dict[str, dict[str, object]]:
        return {
            producer: self._identity_operation(f"{label}-{producer}", producer)
            for producer in RUNTIME_UNUSED_LEAF_IDS
        }

    def _identity_operation(self, label: str, producer: str) -> dict[str, object]:
        exit_code, report = self._moon_operation(label, f"identity:{producer}")
        if exit_code != 0:
            raise RuntimeError(f"identity collection failed for {producer}")
        return report

    @staticmethod
    def _identity_pins(
        identities: dict[str, dict[str, object]],
    ) -> dict[str, dict[str, object]]:
        fields = (
            "sourceManifestSha256",
            "sourceInputsSha256",
            "recipeSha256",
            "toolEnvironmentSha256",
            "inputSha256",
            "identitySha256",
        )
        return {
            producer: {field: identity.get(field) for field in fields}
            for producer, identity in identities.items()
        }

    def _snapshot_outputs(self) -> None:
        if self.lease_path.exists() or self.lease_path.is_symlink():
            raise RuntimeError("selected runtime-script producer is already active")
        git_dir = ROOT / ".git"
        metadata = git_dir.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise RuntimeError("runtime CAS diagnostic requires the canonical Git directory")
        anchor_dir = git_dir / "krr-native-coverage-v1"
        for relative in (*RUNTIME_CAS_OTHER_RECEIPTS, *RUNTIME_CAS_OTHER_JUNIT, RUNTIME_CAS_RECEIPT):
            path = ROOT / relative
            info = path.lstat()
            self.saved_files[path] = (_runtime_cas_safe_file(path), stat.S_IMODE(info.st_mode))
        self.saved_files[self.junit] = (
            _runtime_cas_safe_file(self.junit),
            stat.S_IMODE(self.junit.lstat().st_mode),
        )
        for producer in RUNTIME_UNUSED_LEAF_IDS:
            path = anchor_dir / f"{producer}.json"
            info = path.lstat()
            self.saved_files[path] = (_runtime_cas_safe_file(path), stat.S_IMODE(info.st_mode))

    def _validate_all(self, label: str) -> dict[str, object]:
        exit_code, report = self._moon_operation(label, "validate-all", check=False)
        if exit_code != 0:
            self._host_verify_after_validate_failure(label)
            identities = self._identity_set(f"{label}-identity-probe")
            probe = {
                producer: {
                    "cacheable": identity.get("cacheable"),
                    "missReasons": identity.get("missReasons"),
                    "identityPresent": isinstance(identity.get("identitySha256"), str),
                    "sourceManifestPresent": isinstance(
                        identity.get("sourceManifestSha256"), str
                    ),
                    "sourceInputsPresent": isinstance(
                        identity.get("sourceInputsSha256"), str
                    ),
                }
                for producer, identity in identities.items()
            }
            self.records.append({"identityProbeAfterValidateFailure": probe})
            raise RuntimeError(
                "all runtime-script receipts must validate before mutation; identity probe: "
                + json.dumps(probe, sort_keys=True, separators=(",", ":"))
            )
        if report.get("valid") is not True:
            raise RuntimeError("runtime-script validate-all did not return a valid result")
        return report

    def _host_verify_after_validate_failure(self, label: str) -> None:
        exit_code, _ = self._moon_operation(
            f"{label}-host-verify", "host-verify", check=False
        )
        self.records.append({"hostVerifyAfterValidateFailure": {"actualExit": exit_code}})

    def _moon_run(self, label: str) -> dict[str, object]:
        command = [
            *self.moon_command,
            "run",
            self.task,
            # 事前に標準producer chainを検証済みなので、選択leafだけを再実行する。
            "--upstream",
            "none",
            "--no-actions",
            "--summary",
            "detailed",
            "--color=false",
        ]
        report_path = self.cache_root / "runReport.json"
        previous_report_sha256 = hashlib.sha256(_runtime_cas_safe_file(report_path)).hexdigest()
        exit_code, _ = self._logged(label, command, role="build")
        if exit_code != 0:
            raise RuntimeError(f"Moon producer task failed during {label}")
        result = _runtime_cas_action(self.cache_root, self.run_dir, label)
        if not _runtime_cas_report_is_fresh(previous_report_sha256, result):
            raise RuntimeError(f"Moon run report was not refreshed during {label}")
        self.records.append({"label": label, "exit": exit_code, **result})
        return result

    def _corrupt_blob_moon_attempt(self, corrupted: bytes) -> dict[str, object]:
        command = [
            *self.moon_command,
            "run",
            self.task,
            "--upstream",
            "none",
            "--no-actions",
            "--summary",
            "detailed",
            "--color=false",
        ]
        report_path = self.cache_root / "runReport.json"
        previous_report_sha = hashlib.sha256(_runtime_cas_safe_file(report_path)).hexdigest()
        exit_code, _ = self._logged(
            "moon-corrupt-blob-attempt", command, check=False, role="build"
        )
        result: dict[str, object] = {
            "exit": exit_code,
            "classification": "nonzero_unclassified" if exit_code != 0 else "exit_zero",
        }
        try:
            action = _runtime_cas_action(
                self.cache_root, self.run_dir, "moon-corrupt-blob-attempt"
            )
            report_changed = _runtime_cas_report_is_fresh(previous_report_sha, action)
            result["reportChanged"] = report_changed
            result["sameAction"] = (
                report_changed and action.get("actionHash") == self.action_hash
            )
            result.update(action)
        except (OSError, RuntimeError, ValueError) as error:
            result["classification"] = "run_report_or_output_unavailable"
            result["observationError"] = str(error)
        if exit_code == 0 and result.get("sameAction") is True:
            try:
                receipt = _runtime_cas_safe_file(self.receipt)
                nested_action = moon_json_object(result.get("action"))
                hydration_reported = _runtime_cas_has_cached_hydration(
                    nested_action.get("operations")
                )
                if receipt == corrupted:
                    classification = (
                        "corrupt_blob_hydrated"
                        if hydration_reported
                        else "corrupt_bytes_present_without_hydration_record"
                    )
                elif receipt == self.seed_files[self.receipt][0]:
                    classification = "original_receipt_present"
                else:
                    classification = "different_receipt_present"
                result["classification"] = classification
                result["hydrationReported"] = hydration_reported
                result["corruptBytesMatch"] = receipt == corrupted
            except (OSError, RuntimeError, ValueError) as error:
                result["classification"] = "run_report_or_output_unavailable"
                result["observationError"] = str(error)
        self.records.append({"moonCorruptBlobAttempt": result})
        return result

    def _assert_hydrated(self, action: dict[str, object], label: str) -> None:
        nested_action = moon_json_object(action.get("action"))
        if not _runtime_cas_has_cached_hydration(nested_action.get("operations")):
            raise RuntimeError(f"Moon did not report cached output hydration: {label}")

    def _remove_outputs(self) -> None:
        for path in self.outputs:
            if path.is_symlink() or not path.is_file():
                raise RuntimeError("selected producer output changed before removal")
            path.unlink()

    def _capture_repaired_outputs(self) -> None:
        self.repaired_files = {
            path: (_runtime_cas_safe_file(path), stat.S_IMODE(path.lstat().st_mode))
            for path in self.outputs
        }

    def _assert_manifest_unchanged(self) -> None:
        if self.manifest_path is None or self.manifest_bytes is None or self.manifest_mode is None:
            raise RuntimeError("selected Moon CAS manifest was not pinned")
        if (
            _runtime_cas_safe_file(self.manifest_path) != self.manifest_bytes
            or stat.S_IMODE(self.manifest_path.lstat().st_mode) != self.manifest_mode
        ):
            raise RuntimeError("selected Moon CAS manifest changed during the diagnostic")

    def _restore_outputs(self) -> None:
        selected = {
            path: snapshot
            for path, snapshot in self.saved_files.items()
            if path in self.outputs or path.name == f"{RUNTIME_CAS_PRODUCER}.json"
        }
        restored = _runtime_cas_restore_entries(selected)
        if len(restored) != 3:
            raise RuntimeError("runtime-script diagnostic did not restore its three owned outputs")

    def _restore_blob(self) -> bool:
        if self.blob_fd is None or self.blob_bytes is None or self.blob_path is None:
            return True
        if self.blob_mode is None:
            raise RuntimeError("selected Moon CAS blob mode was not pinned")
        info = self.blob_path.lstat()
        opened = os.fstat(self.blob_fd)
        if (
            stat.S_ISLNK(info.st_mode)
            or not stat.S_ISREG(info.st_mode)
            or info.st_ino != opened.st_ino
            or info.st_dev != opened.st_dev
        ):
            raise RuntimeError("selected Moon CAS blob was replaced during the diagnostic")
        os.fchmod(self.blob_fd, self.blob_mode)
        os.lseek(self.blob_fd, 0, os.SEEK_SET)
        os.ftruncate(self.blob_fd, 0)
        _runtime_cas_write_fd(self.blob_fd, self.blob_bytes)
        os.lseek(self.blob_fd, 0, os.SEEK_SET)
        restored = os.read(self.blob_fd, len(self.blob_bytes) + 1)
        return (
            hashlib.sha256(restored).hexdigest()
            == hashlib.sha256(self.blob_bytes).hexdigest()
            and stat.S_IMODE(os.fstat(self.blob_fd).st_mode) == self.blob_mode
        )

    def _baseline(self) -> dict[str, object]:
        self._snapshot_outputs()
        self._validate_all("validate-before")
        report = moon_json_object(json.loads(self.saved_files[self.receipt][0]))
        if report.get("producer") != RUNTIME_CAS_PRODUCER:
            raise RuntimeError("selected receipt belongs to another producer")
        reports = report.get("junitReports")
        if not isinstance(reports, list) or len(reports) != 1:
            raise RuntimeError("selected receipt does not contain one JUnit group")
        if moon_json_object(reports[0]).get("path") != RUNTIME_CAS_JUNIT:
            raise RuntimeError("selected receipt points outside its fixed output")
        return report

    def _seed_and_restore(self) -> dict[str, object]:
        first = self._moon_run("moon-seed")
        self.seed_files = _runtime_cas_snapshot_files(self.outputs)
        self.seed_report = moon_json_object(json.loads(self.seed_files[self.receipt][0]))
        second = self._moon_run("moon-same-input-hit")
        if first.get("actionHash") != second.get("actionHash"):
            raise RuntimeError("same-input Moon action hash changed")
        self.action_hash = str(second["actionHash"])
        self._assert_hydrated(second, "same-input")
        if not _runtime_cas_files_match_snapshot(self.seed_files):
            raise RuntimeError("successful Moon cache hit changed seeded output bytes")
        third = self._manifest_and_blob(second["actionHash"])
        self._remove_outputs()
        restored = self._moon_run("moon-deleted-output-hydration")
        if restored.get("actionHash") != self.action_hash:
            raise RuntimeError("deleted-output hydration used another Moon action")
        self._assert_hydrated(restored, "deleted-output")
        if not _runtime_cas_files_match_snapshot(self.seed_files):
            raise RuntimeError("Moon did not restore the seeded output bytes")
        return third

    def _manifest_and_blob(self, action_hash: object) -> dict[str, object]:
        if not isinstance(action_hash, str):
            raise RuntimeError("selected Moon action hash is invalid")
        manifest, blob, content, entry = _runtime_cas_manifest_blob(
            self.cache_root, action_hash, RUNTIME_CAS_RECEIPT
        )
        blob_digest = moon_json_object(entry.get("digest")).get("hash")
        if not isinstance(blob_digest, str):
            raise RuntimeError("selected CAS digest is invalid")
        owners = _runtime_cas_unique_blob_owner(
            self.cache_root, blob_digest, manifest, RUNTIME_CAS_RECEIPT
        )
        manifest_info = manifest.lstat()
        self.manifest_path = manifest
        self.manifest_bytes = _runtime_cas_safe_file(manifest)
        self.manifest_mode = stat.S_IMODE(manifest_info.st_mode)
        self.blob_path, self.blob_bytes = blob, content
        info = blob.lstat()
        self.blob_mode = stat.S_IMODE(info.st_mode)
        descriptor = os.open(blob, os.O_RDWR | getattr(os, "O_NOFOLLOW", 0))
        if os.fstat(descriptor).st_ino != info.st_ino:
            os.close(descriptor)
            raise RuntimeError("Moon CAS blob changed while opening")
        self.blob_fd = descriptor
        return {
            "actionHash": action_hash,
            "manifest": str(manifest),
            "manifestSha256": digest(manifest),
            "blob": str(blob),
            "blobSha256": blob_digest,
            "blobOwnerCount": owners,
        }

    def _seed_repaired_epoch(
        self, raw_root: Path
    ) -> tuple[str, set[str], dict[Path, tuple[bytes, int]], dict[str, object] | None]:
        repaired = moon_json_object(json.loads(self.repaired_files[self.receipt][0]))
        raw_before = _runtime_cas_raw_directories(raw_root)
        seed = self._moon_run("moon-repaired-epoch-seed")
        new_hash = seed.get("actionHash")
        if not isinstance(new_hash, str) or new_hash == self.action_hash:
            raise RuntimeError("repaired epoch did not use a new Moon action")
        raw_after = _runtime_cas_raw_directories(raw_root)
        if not raw_before.issubset(raw_after):
            raise RuntimeError("producer logs disappeared during repaired epoch seed")
        added = raw_after - raw_before
        seed_log = None
        if added:
            seed_log = _runtime_cas_new_producer_log(
                raw_root, raw_before, _runtime_cas_expected_command()
            )
        else:
            self._assert_hydrated(seed, "repaired-epoch-seed")
        self._validate_all("validate-repaired-epoch-seed")
        seeded = _runtime_cas_snapshot_files(self.outputs)
        seeded_receipt = moon_json_object(json.loads(seeded[self.receipt][0]))
        if any(seeded_receipt.get(key) != repaired.get(key) for key in ("identitySha256", "epoch")):
            raise RuntimeError("repaired epoch seed changed the validated identity or epoch")
        return new_hash, raw_after, seeded, seed_log

    def _verify_repaired_epoch_cache(self, raw_root: Path) -> dict[str, object]:
        new_hash, raw_after, seeded, seed_log = self._seed_repaired_epoch(raw_root)
        for label in ("moon-repaired-epoch-hit", "moon-repaired-epoch-restore"):
            if label.endswith("restore"):
                self._remove_outputs()
            action = self._moon_run(label)
            if action.get("actionHash") != new_hash:
                raise RuntimeError("repaired epoch cache action changed")
            self._assert_hydrated(action, label)
            if not _runtime_cas_files_match_snapshot(seeded):
                raise RuntimeError("repaired epoch cache output differs from its seed")
            if _runtime_cas_raw_directories(raw_root) != raw_after:
                raise RuntimeError("repaired epoch cache hit created a producer log")
            self._validate_all(f"validate-{label}")
        return {
            "actionHash": new_hash,
            "seedProducerLog": seed_log,
            "receiptSha256": hashlib.sha256(seeded[self.receipt][0]).hexdigest(),
            "junitSha256": hashlib.sha256(seeded[self.junit][0]).hexdigest(),
            "hitAndRestoreValidated": True,
        }

    def _corrupt_and_recover(self) -> dict[str, object]:
        if self.blob_fd is None or self.blob_bytes is None:
            raise RuntimeError("selected CAS blob was not pinned")
        corrupt = bytearray(self.blob_bytes)
        if len(corrupt) < 32:
            raise RuntimeError("selected receipt blob is too small to corrupt")
        corrupt[len(corrupt) // 2] ^= 1
        os.lseek(self.blob_fd, 0, os.SEEK_SET)
        os.ftruncate(self.blob_fd, 0)
        _runtime_cas_write_fd(self.blob_fd, bytes(corrupt))
        self._remove_outputs()
        raw_root = ROOT / "tmp/quality/runtime-script-results/raw"
        raw_directories = _runtime_cas_raw_directories(raw_root)
        moon_attempt = self._corrupt_blob_moon_attempt(bytes(corrupt))
        outputs_absent = _runtime_cas_outputs_absent(self.outputs)
        producer_logs_unchanged = _runtime_cas_raw_directories(raw_root) == raw_directories
        probe_acceptance = _runtime_cas_probe_acceptance(
            moon_attempt, outputs_absent, producer_logs_unchanged
        )
        moon_attempt["probeAcceptance"] = probe_acceptance
        moon_attempt["ownedOutputsAbsent"] = outputs_absent
        moon_attempt["producerLogsUnchanged"] = producer_logs_unchanged
        if probe_acceptance == "controlled_failed_closed":
            moon_attempt["classification"] = probe_acceptance
        moon_attempt["repairInputMode"] = _runtime_cas_repair_input_mode(
            moon_attempt, outputs_absent
        )
        if self.lease_path.exists() or self.lease_path.is_symlink():
            raise RuntimeError("selected runtime-script producer lease is unexpectedly active")
        exit_code, repaired = self._moon_operation("ensure-all-repair", "ensure-all")
        if exit_code != 0:
            raise RuntimeError("uncached KRR ensure-all did not repair the corrupted leaf")
        if repaired.get("valid") is not True or type(repaired.get("recovered")) is not bool:
            raise RuntimeError("KRR ensure-all did not return a valid recovery result")
        self._assert_manifest_unchanged()
        expected_command = _runtime_cas_expected_command()
        producer_log = _runtime_cas_new_producer_log(
            raw_root, raw_directories, expected_command
        )
        if self.lease_path.exists() or self.lease_path.is_symlink():
            raise RuntimeError("runtime-script producer lease remained after recovery")
        self._capture_repaired_outputs()
        if not self._restore_blob():
            raise RuntimeError("original Moon CAS blob was not restored before control")
        self._validate_all("validate-repaired-before-new-seed")
        control = self._verify_repaired_epoch_cache(raw_root)
        self._assert_manifest_unchanged()
        if probe_acceptance is None:
            raise RuntimeError("corrupted Moon CAS probe was not an accepted classification")
        return {
            "targetOutput": RUNTIME_CAS_RECEIPT,
            "hydratedCorruptSha256": hashlib.sha256(corrupt).hexdigest(),
            "moonAttempt": moon_attempt,
            "recoveryResult": repaired,
            "producerLog": producer_log,
            "repairedEpochCache": control,
            "probeAcceptance": probe_acceptance,
        }

    def _assert_repaired(
        self, baseline: dict[str, object], identities_before: dict[str, dict[str, object]]
    ) -> dict[str, object]:
        self._validate_all("validate-after-repair")
        final = moon_json_object(json.loads(_runtime_cas_safe_file(self.receipt)))
        repair_baseline = self.seed_report if self.seed_report is not None else baseline
        if (
            final.get("identitySha256") != repair_baseline.get("identitySha256")
            or final.get("epoch") == repair_baseline.get("epoch")
        ):
            raise RuntimeError("repaired receipt did not use a fresh epoch for same inputs")
        for relative in (*RUNTIME_CAS_OTHER_RECEIPTS, *RUNTIME_CAS_OTHER_JUNIT):
            path = ROOT / relative
            if (_runtime_cas_safe_file(path) != self.saved_files[path][0]
                    or stat.S_IMODE(path.lstat().st_mode) != self.saved_files[path][1]):
                raise RuntimeError("ensure-all modified an unrelated producer output")
        for path, (content, _) in self.saved_files.items():
            if (
                path.parent.name == "krr-native-coverage-v1"
                and path.name != f"{RUNTIME_CAS_PRODUCER}.json"
                and _runtime_cas_safe_file(path) != content
            ):
                raise RuntimeError("ensure-all modified an unrelated trusted anchor")
        identities_after = self._identity_set("identity-after")
        for producer in RUNTIME_UNUSED_LEAF_IDS:
            before = identities_before[producer]
            after = identities_after[producer]
            if any(
                before.get(field) != after.get(field)
                for field in ("identitySha256", "inputSha256", "sourceManifestSha256")
            ):
                raise RuntimeError("runtime-script source identity changed during diagnostic")
        return {
            "selectedFreshEpoch": True,
            "unrelatedReceiptsUnchanged": True,
            "sourceIdentitiesUnchanged": True,
            "identityPinsAfter": self._identity_pins(identities_after),
            "receiptSha256": hashlib.sha256(_runtime_cas_safe_file(self.receipt)).hexdigest(),
            "junitSha256": hashlib.sha256(_runtime_cas_safe_file(self.junit)).hexdigest(),
        }

    def execute(self) -> int:
        status = "failed"
        error: str | None = None
        try:
            baseline = self._baseline()
            identities_before = self._identity_set("identity-before")
            self.records.append(
                {
                    "baseline": {
                        "receiptSha256": hashlib.sha256(
                            self.saved_files[self.receipt][0]
                        ).hexdigest(),
                        "junitSha256": hashlib.sha256(
                            self.saved_files[self.junit][0]
                        ).hexdigest(),
                        "selectedIdentitySha256": baseline.get("identitySha256"),
                        "selectedEpoch": baseline.get("epoch"),
                        "identityPinsBefore": self._identity_pins(identities_before),
                    }
                }
            )
            cache_record = self._seed_and_restore()
            corruption = self._corrupt_and_recover()
            repaired = self._assert_repaired(baseline, identities_before)
            self.records.append(
                {
                    "identityPinsBefore": self._identity_pins(identities_before),
                    "cache": cache_record,
                    "corruption": corruption,
                    "repair": repaired,
                }
            )
            self.passed = True
            status = "passed"
        except Exception as exception:
            error = str(exception)
        finally:
            restore_errors: list[str] = []
            outputs_restored = False
            try:
                blob_restored = self._restore_blob()
            except Exception as exception:
                blob_restored = False
                restore_errors.append(f"blob: {exception}")
            try:
                self._restore_outputs()
                outputs_restored = True
            except Exception as exception:
                restore_errors.append(f"outputs: {exception}")
            try:
                if self.lease_path.exists() or self.lease_path.is_symlink():
                    raise RuntimeError("runtime-script producer lease remains active")
            except Exception as exception:
                restore_errors.append(f"lease: {exception}")
            if self.blob_fd is not None:
                os.close(self.blob_fd)
            if not blob_restored or restore_errors:
                status = "failed"
                error = "; ".join(restore_errors) or "selected Moon CAS blob was not restored"
            report = {
                "schema": "krr-runtime-script-moon-cas-diagnostic-v2",
                "status": status,
                "fixtureOnly": False,
                "krrCoverageAchievement": False,
                "producer": RUNTIME_CAS_PRODUCER,
                "task": RUNTIME_CAS_TASK,
                "casConfiguration": {
                    "requested": {
                        "verifyIntegrity": self.environment["MOON_CACHE_CAS_VERIFY_INTEGRITY"],
                        "casOutputsCache": self.environment["MOON_EXPERIMENT_CAS_OUTPUTS_CACHE"],
                        "cacheMode": self.environment["MOON_CACHE"],
                    },
                    "effectiveReadback": "unavailable_from_current_moon_run_report",
                },
                "mutationRationale": (
                    "The selected producer receipt is the uniquely owned CAS output used by the "
                    "strict KRR validator; its JUnit output remains separately pinned and is "
                    "not the corrupted object."
                ),
                "steps": self.records,
                "casBlobRestored": blob_restored,
                "priorOutputsRestored": outputs_restored,
                "restoreErrors": restore_errors,
                "error": error,
            }
            report_path = self.run_dir / "runtime-script-moon-cas-diagnostic.json"
            report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
            print(report_path)
        return 0 if status == "passed" else 1
