"""Validate the authenticated transport before considering remote evidence."""
from __future__ import annotations

from pathlib import Path

import html_performance_remote_evidence as evidence
import html_performance_attestation as attestation
import verify_html_performance_evidence as gate


_REMOTE_CONTEXT_AUTHORITY = object()


class RemoteEvidenceContext:
    __slots__ = ("head", "manifest_sha256", "builds")

    def __init__(self, head: str, manifest_sha256: str, builds: dict[str, gate.JsonObject],
                 *, authority: object) -> None:
        gate.require(authority is _REMOTE_CONTEXT_AUTHORITY, "remote evidence context is unauthenticated")
        self.head = head
        self.manifest_sha256 = manifest_sha256
        self.builds = builds


def verify_authenticated_remote(directory: Path, archive: Path, root: Path, repository: str,
                                run: gate.JsonObject, artifact_name: str, head: str) -> gate.JsonObject:
    """Authenticate the producer artifact and independently run the formal gate."""
    attestation.verify_remote_attestation(archive, root, repository, run, artifact_name, head)
    evidence.extract_remote_transport(directory, archive, head)
    producer_result = evidence.validate_producer_verification_receipt(directory, head)
    transported = gate.Bundle(directory)
    attempts = gate._objects(transported.manifest.get("attempts"), "remote attempts")
    first = transported.json(attempts[0]["expected"])
    gate._require_catalog_attempt_set(attempts, first)
    candidate = gate._object(gate._object(first.get("arms"), "remote arms").get("candidate"),
                             "remote candidate arm")
    build = evidence.validate_remote_candidate_build(transported, root, candidate.get("build_artifact_ref"), head)
    gate.require(candidate.get("binary_sha256") == build["binarySha256"]
                 and candidate.get("source_snapshot_sha256") == build["sourceSnapshotSha256"]
                 and candidate.get("lock_sha256") == gate.LOCK_SHA,
                 "remote candidate arm differs from its build proof")
    contract = gate._object(first.get("referenceParent", {}), "remote reference contract")
    if contract.get("referenceKind") == "fresh-history-source":
        from html_performance_fresh_reference import validate_remote_fresh_reference
        baseline = validate_remote_fresh_reference(transported, root, first, head)
        gate.require(all(baseline.get(key) == build.get(key) for key in (
            "cargoExecutableSha256", "rustcExecutableSha256", "buildEnvironmentSha256",
            "toolVersions", "cargoConfigPins", "cargoLauncher", "rustcLauncher", "guardPython")),
            "remote fresh reference and candidate build environments differ")
    else:
        baseline = evidence.validate_remote_history_baseline(transported, root, first, head)
    context = RemoteEvidenceContext(head, gate.sha256(directory / "bundle.json"),
                                    {"baseline": baseline, "candidate": build},
                                    authority=_REMOTE_CONTEXT_AUTHORITY)
    result = gate._verify(directory, root, require_catalog=True, remote_context=context)
    gate._require_formal_release_result(result)
    gate.require(result == producer_result, "remote formal result differs from producer verification")
    return result
