"""Verify one anchored runtime-script result without trusting cache metadata."""

from __future__ import annotations

import json
from pathlib import Path
from typing import cast

import native_coverage_local_anchor as local_anchor
import runtime_script_junit_outputs as junit_outputs
import runtime_script_result as result


def verify_receipt(
    repo: Path,
    producer: str,
    identity_sha256: str,
    epoch: str,
    receipt_path: Path,
) -> dict[str, object]:
    if producer not in result.PRODUCERS:
        raise result.RuntimeScriptResultError("unknown runtime-script producer")
    result._require_digest(identity_sha256, "identity digest")
    expected_path = repo / result.RECEIPT_DIRECTORY / f"{producer}.json"
    if receipt_path != expected_path:
        raise result.RuntimeScriptResultError("receipt path is outside fixed producer output")
    try:
        relative = receipt_path.relative_to(repo).as_posix()
        receipt_bytes = junit_outputs.read_output(repo, relative, producer, ".json")
        receipt = json.loads(receipt_bytes)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        if isinstance(error, result.RuntimeScriptResultError):
            raise
        raise result.RuntimeScriptResultError("receipt is invalid or unavailable") from error
    result._validate_receipt_content(repo, receipt, producer, identity_sha256, epoch)
    if not local_anchor.verify(repo, producer, identity_sha256, epoch, receipt_bytes):
        raise result.RuntimeScriptResultError("trusted local anchor does not match receipt")
    return cast(dict[str, object], receipt)
