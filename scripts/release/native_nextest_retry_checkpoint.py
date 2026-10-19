"""Bind a root Nextest retry checkpoint to the existing private local anchor."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import cast

import native_coverage_local_anchor as local_anchor
import native_coverage_local_result as local_result
import native_nextest_retry_artifacts as artifacts

PRODUCER = "root-nextest-retry"
SCHEMA = "krr-native-nextest-retry-checkpoint-v1"
CHECKPOINT_PATH = Path("tmp/quality/standalone-performance-coverage/root-nextest-retry.json")
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
RUN_ID_RE = re.compile(
    r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\Z"
)
CHECKPOINT_KEYS = {
    "schema",
    "identitySha256",
    "anchorEpoch",
    "nextestRunId",
    "testExit",
    "snapshot",
}


def _valid_identity(value: object) -> bool:
    return isinstance(value, str) and SHA256_RE.fullmatch(value) is not None


def _valid_run_id(value: object) -> bool:
    return isinstance(value, str) and RUN_ID_RE.fullmatch(value) is not None


def _read_payload(repo: Path) -> tuple[dict[str, object], bytes] | None:
    try:
        receipt = local_result._read_json_bytes(
            repo / CHECKPOINT_PATH, "Nextest retry checkpoint"
        )
        value = json.loads(receipt)
    except (OSError, ValueError, UnicodeDecodeError, json.JSONDecodeError):
        return None
    return (value, receipt) if isinstance(value, dict) else None


def _valid_payload(value: dict[str, object], identity_sha256: str) -> bool:
    if (
        set(value) != CHECKPOINT_KEYS
        or value.get("schema") != SCHEMA
        or value.get("identitySha256") != identity_sha256
        or not _valid_identity(value.get("identitySha256"))
        or not isinstance(value.get("anchorEpoch"), str)
        or SHA256_RE.fullmatch(cast(str, value["anchorEpoch"])) is None
        or not _valid_run_id(value.get("nextestRunId"))
        or type(value.get("testExit")) is not int
        or value.get("testExit") not in {0, 100}
        or not isinstance(value.get("snapshot"), dict)
    ):
        return False
    snapshot = value.get("snapshot")
    return isinstance(snapshot, dict) and snapshot.get("runId") == value["nextestRunId"]


def read_checkpoint(repo: Path, identity_sha256: str) -> dict[str, object] | None:
    if not _valid_identity(identity_sha256):
        return None
    try:
        local_result._ensure_report_directory(repo)
        stored = _read_payload(repo)
        if stored is None:
            return None
        value, receipt = stored
        if not _valid_payload(value, identity_sha256):
            return None
        if not local_anchor.verify(
            repo,
            PRODUCER,
            identity_sha256,
            cast(str, value["anchorEpoch"]),
            receipt,
        ):
            return None
        if not artifacts.verify_snapshot(repo, value["snapshot"]):
            return None
        return value
    except (OSError, ValueError):
        return None


def write_checkpoint(
    repo: Path, identity_sha256: str, run_id: str, test_exit: int
) -> bool:
    if (
        not _valid_identity(identity_sha256)
        or not _valid_run_id(run_id)
        or type(test_exit) is not int
        or test_exit not in {0, 100}
    ):
        return False
    try:
        local_result._ensure_report_directory(repo)
        snapshot = artifacts.snapshot(repo, run_id)
        if not isinstance(snapshot, dict) or snapshot.get("runId") != run_id:
            return False
        epoch = local_anchor.prepare(repo, PRODUCER, identity_sha256)
        if epoch is None:
            return False
        payload: dict[str, object] = {
            "schema": SCHEMA,
            "identitySha256": identity_sha256,
            "anchorEpoch": epoch,
            "nextestRunId": run_id,
            "testExit": test_exit,
            "snapshot": snapshot,
        }
        path = repo / CHECKPOINT_PATH
        local_result._write_atomic(path, payload)
        receipt = local_result._read_json_bytes(path, "Nextest retry checkpoint")
        if not local_anchor.commit(repo, PRODUCER, identity_sha256, epoch, receipt):
            local_anchor.invalidate(repo, PRODUCER, identity_sha256, epoch)
            return False
        return True
    except (OSError, ValueError):
        return False


def invalidate_checkpoint(repo: Path, identity_sha256: str) -> None:
    if not _valid_identity(identity_sha256):
        return
    epoch = local_anchor.prepare(repo, PRODUCER, identity_sha256)
    if epoch is not None:
        local_anchor.invalidate(repo, PRODUCER, identity_sha256, epoch)
    path = repo / CHECKPOINT_PATH
    try:
        if not path.is_symlink() and path.is_file():
            path.unlink()
    except OSError:
        pass
