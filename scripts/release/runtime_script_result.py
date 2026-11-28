"""Validate receipts for the two fixed runtime-script Moon leaves."""

from __future__ import annotations

import hashlib
import json
import os
import re
import secrets
import stat
from pathlib import Path

import native_coverage_local_anchor as local_anchor
import runtime_script_junit as junit
import runtime_script_junit_publish as junit_publish
import runtime_script_profiles as profiles

SCHEMA = "krr-runtime-script-result-v2"
PRODUCERS = {
    "runtime-asset-script-other",
    *profiles.accepted_producers(),
}
RECEIPT_DIRECTORY = "tmp/quality/runtime-script-results"
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")


class RuntimeScriptResultError(ValueError):
    """Raised when a fixed runtime-script result cannot be trusted."""


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _canonical_json(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode()


def _require_digest(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        raise RuntimeScriptResultError(f"{label} must be a lowercase SHA-256")
    return value


def build_identity(
    producer: str,
    input_sha256: str,
) -> str:
    """Bind one fixed producer to the caller's complete pre-run input digest."""
    if producer not in PRODUCERS:
        raise RuntimeScriptResultError("unknown runtime-script producer")
    _require_digest(input_sha256, "producer input digest")
    identity = {
        "schema": SCHEMA,
        "producer": producer,
        "inputSha256": input_sha256,
    }
    return _sha256(_canonical_json(identity))


def make_receipt(
    producer: str,
    identity_sha256: str,
    epoch: str,
    junit_reports: list[tuple[str, bytes]],
    process_exit: int,
) -> bytes:
    if producer not in PRODUCERS:
        raise RuntimeScriptResultError("unknown runtime-script producer")
    _require_digest(identity_sha256, "identity digest")
    _require_digest(epoch, "run epoch")
    if process_exit != 0:
        raise RuntimeScriptResultError("failed test processes cannot produce receipts")
    try:
        reports = junit.build_current_records(junit_reports, producer)
    except junit.RuntimeScriptJUnitError as error:
        raise RuntimeScriptResultError(str(error)) from error
    return _canonical_json(
        {
            "schema": SCHEMA,
            "producer": producer,
            "identitySha256": identity_sha256,
            "epoch": epoch,
            "junitReports": reports,
        }
    )


def begin_fresh_run(repo: Path, producer: str, identity_sha256: str) -> str | None:
    """Invalidate an older same-input success before starting a requested rerun."""
    if producer not in PRODUCERS:
        raise RuntimeScriptResultError("unknown runtime-script producer")
    _require_digest(identity_sha256, "identity digest")
    previous_epoch = local_anchor.prepare(repo, producer, identity_sha256)
    if previous_epoch is None:
        return None
    local_anchor.invalidate(repo, producer, identity_sha256, previous_epoch)
    return local_anchor.prepare(repo, producer, identity_sha256)


def commit_receipt(
    repo: Path,
    producer: str,
    identity_sha256: str,
    epoch: str,
    receipt_path: Path,
    receipt_bytes: bytes,
) -> None:
    expected_path = repo / RECEIPT_DIRECTORY / f"{producer}.json"
    if producer not in PRODUCERS or receipt_path != expected_path:
        raise RuntimeScriptResultError("receipt path is outside the fixed producer output")
    verify_shape_only = json.loads(receipt_bytes)
    if (
        not isinstance(verify_shape_only, dict)
        or verify_shape_only.get("epoch") != epoch
        or verify_shape_only.get("producer") != producer
        or verify_shape_only.get("identitySha256") != identity_sha256
    ):
        raise RuntimeScriptResultError("receipt does not match requested producer identity")
    _validate_receipt_content(repo, verify_shape_only, producer, identity_sha256, epoch)
    try:
        parent = expected_path.parent
        current = repo
        for part in Path(RECEIPT_DIRECTORY).parts:
            current = current / part
            try:
                current.mkdir(mode=0o700)
            except FileExistsError:
                pass
            metadata = current.lstat()
            if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
                raise RuntimeScriptResultError("receipt output directory is not private")
        parent.resolve(strict=True).relative_to(repo.resolve(strict=True))
        temporary_path = parent / f".{expected_path.name}.{secrets.token_hex(12)}.tmp"
        descriptor = os.open(
            temporary_path,
            os.O_WRONLY | os.O_CREAT | os.O_TRUNC | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
        with os.fdopen(descriptor, "wb") as destination:
            metadata = os.fstat(destination.fileno())
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                raise RuntimeScriptResultError("receipt output is not a regular file")
            os.fchmod(destination.fileno(), 0o600)
            destination.write(receipt_bytes)
            destination.flush()
            os.fsync(destination.fileno())
        os.replace(temporary_path, expected_path)
    except OSError as error:
        raise RuntimeScriptResultError("receipt output could not be written safely") from error
    if not local_anchor.commit(repo, producer, identity_sha256, epoch, receipt_bytes):
        raise RuntimeScriptResultError("trusted local anchor commit failed")


def _validate_receipt_content(
    repo: Path, receipt: object, producer: str, identity_sha256: str, epoch: str
) -> None:
    if (
        not isinstance(receipt, dict)
        or set(receipt)
        != {"schema", "producer", "identitySha256", "epoch", "junitReports"}
        or receipt.get("schema") != SCHEMA
        or receipt.get("producer") != producer
        or receipt.get("identitySha256") != identity_sha256
        or receipt.get("epoch") != epoch
        or not isinstance(receipt.get("epoch"), str)
        or not isinstance(receipt.get("junitReports"), list)
    ):
        raise RuntimeScriptResultError("receipt has an unsupported shape")
    try:
        junit_publish.validate_current_records(repo, receipt["junitReports"], producer)
    except junit.RuntimeScriptJUnitError as error:
        raise RuntimeScriptResultError(str(error)) from error
