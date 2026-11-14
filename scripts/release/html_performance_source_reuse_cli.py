"""Validate HTML performance source reuse and create an immutable local request."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
import tempfile
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from typing import TypedDict

import verify_html_performance_evidence as gate
from html_performance_memory_policy import REASSESSMENT_POLICY_VERSIONS, PREVIOUS_POLICY_VERSION

SCHEMA = "krr-html-performance-source-reuse-request-v1"
ELIGIBILITY_SCHEMA = "krr-html-performance-source-reuse-eligibility-v2"
ARTIFACT_REUSE_KINDS = ("test-source-artifact-equivalence-v1",
                        "program-source-artifact-equivalence-v1",
                        "binary-bound-product-source-equivalence-v1")


class ReuseRequest(TypedDict):
    schema: str
    measuredDir: str
    measuredManifestSha256: str
    currentBuildDir: str
    currentBuildManifestSha256: str
    currentBuildArtifactPath: str
    currentBuildArtifactSha256: str


def _directory(value: object) -> str:
    if not isinstance(value, str):
        raise ValueError("bundle directory must be text")
    path = Path(value)
    if not path.is_absolute() or path.resolve(strict=True) != path or not path.is_dir():
        raise ValueError("bundle directory must be canonical and absolute")
    return value


def _sha(value: object) -> str:
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise ValueError("invalid SHA256")
    return value


def _request(value: object) -> ReuseRequest:
    keys = {"schema", "measuredDir", "measuredManifestSha256", "currentBuildDir",
            "currentBuildManifestSha256", "currentBuildArtifactPath", "currentBuildArtifactSha256"}
    if not isinstance(value, dict) or set(value) != keys or value.get("schema") != SCHEMA:
        raise ValueError("invalid source reuse request schema")
    artifact = value["currentBuildArtifactPath"]
    if not isinstance(artifact, str) or not artifact or Path(artifact).is_absolute():
        raise ValueError("invalid build artifact path")
    artifact_path = Path(artifact)
    if ".." in artifact_path.parts or str(artifact_path) != artifact:
        raise ValueError("invalid build artifact path")
    return {"schema": SCHEMA, "measuredDir": _directory(value["measuredDir"]),
            "measuredManifestSha256": _sha(value["measuredManifestSha256"]),
            "currentBuildDir": _directory(value["currentBuildDir"]),
            "currentBuildManifestSha256": _sha(value["currentBuildManifestSha256"]),
            "currentBuildArtifactPath": artifact,
            "currentBuildArtifactSha256": _sha(value["currentBuildArtifactSha256"])}


@contextmanager
def _new_git_query(parent: Path) -> Iterator[gate.GitQuery]:
    from produce_html_performance_evidence import _make_git_query

    # 照会ログの物理pinはこの認証中だけ使い、結果へ持ち出さない。
    with tempfile.TemporaryDirectory(prefix="source-reuse-git-", dir=parent) as directory:
        scratch = Path(directory).resolve()
        yield _make_git_query(scratch, scratch_root=scratch)


def _validate(request: ReuseRequest, root: Path, *, git_query: gate.GitQuery,
              memory_policy_version: str | None = None) -> gate.JsonObject:
    return gate.verify_source_reuse(
        Path(request["measuredDir"]), root, request["measuredManifestSha256"],
        Path(request["currentBuildDir"]), request["currentBuildManifestSha256"],
        request["currentBuildArtifactPath"], request["currentBuildArtifactSha256"],
        git_query=git_query, **gate._memory_policy_kwargs(memory_policy_version),
    )


def _request_path(path: Path, root: Path, *, existing: bool) -> Path:
    root = root.resolve(strict=True)
    tmp = root / "tmp"
    if not path.is_absolute() or path.parent.resolve(strict=True) != path.parent:
        raise ValueError("request path must be canonical and absolute")
    if not path.is_relative_to(tmp) or path == tmp:
        raise ValueError("request must be inside repository tmp")
    if existing:
        metadata = path.lstat()
        if not stat.S_ISREG(metadata.st_mode) or path.is_symlink() or path.resolve() != path:
            raise ValueError("request must be a canonical regular file")
    return path


def _stamp(metadata: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (metadata.st_dev, metadata.st_ino, metadata.st_mode, metadata.st_size,
            metadata.st_mtime_ns, metadata.st_ctime_ns)


def _eligibility_matches(stored: object, current: gate.JsonObject | None) -> bool:
    if stored == current:
        return True
    if not isinstance(stored, dict) or current is None:
        return False
    old_kind = stored.get("reuseKind")
    if (old_kind not in {"test-source-artifact-equivalence-v1",
                         "program-source-artifact-equivalence-v1"}
            or current.get("reuseKind") != "binary-bound-product-source-equivalence-v1"):
        return False
    # WHY: 再認証結果の名称だけを比較時に読み替え、過去の記録は変更しない。
    return {**stored, "reuseKind": current["reuseKind"]} == current


def verify_reuse_request(request_path: Path, root: Path, *,
                         expected_measured_dir: Path | None = None,
                         expected_measured_manifest_sha256: str | None = None,
                         expected_request_sha256: str | None = None,
                         memory_policy_version: str | None = None) -> gate.JsonObject:
    path = _request_path(request_path, root, existing=True)
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(descriptor, "rb") as stream:
        metadata = os.fstat(stream.fileno())
        current = path.lstat()
        if not stat.S_ISREG(metadata.st_mode) or _stamp(metadata) != _stamp(current):
            raise ValueError("request file changed during open")
        raw = stream.read()
        if expected_request_sha256 is not None:
            _sha(expected_request_sha256)
            if hashlib.sha256(raw).hexdigest() != expected_request_sha256:
                raise ValueError("request digest differs")
        value = json.loads(raw.decode("utf8"), object_pairs_hook=gate.unique_object)
        eligibility = None
        has_eligibility = isinstance(value, dict) and value.get("schema") == ELIGIBILITY_SCHEMA
        if has_eligibility and isinstance(value, dict):
            if set(value) != {"schema", "request", "result"}:
                raise ValueError("invalid eligibility schema")
            eligibility = value.get("result")
            value = value.get("request")
        request = _request(value)
        if (expected_measured_dir is not None
                and Path(request["measuredDir"]) != expected_measured_dir.resolve(strict=True)):
            raise ValueError("request measured bundle differs")
        if (expected_measured_manifest_sha256 is not None
                and request["measuredManifestSha256"] != _sha(expected_measured_manifest_sha256)):
            raise ValueError("request measured manifest differs")
        original_version = None
        if has_eligibility and isinstance(eligibility, dict):
            measured = eligibility.get("measuredResult")
            accepted_versions = (PREVIOUS_POLICY_VERSION, *REASSESSMENT_POLICY_VERSIONS)
            if isinstance(measured, dict) and measured.get("memoryPolicyVersion") in accepted_versions:
                original_version = measured["memoryPolicyVersion"]
                if not isinstance(original_version, str):
                    raise ValueError("invalid stored memory policy version")
        with _new_git_query(path.parent) as git_query:
            original = (_validate(request, root.resolve(strict=True), git_query=git_query,
                                  memory_policy_version=original_version) if has_eligibility else None)
            if has_eligibility and (not _eligibility_matches(eligibility, original) or original is None
                    or original.get("reuseKind") not in ARTIFACT_REUSE_KINDS):
                raise ValueError("stored eligibility differs from revalidation")
            result = (original if original is not None and memory_policy_version == original_version else
                      _validate(request, root.resolve(strict=True), git_query=git_query,
                                memory_policy_version=memory_policy_version)
                      if memory_policy_version is not None or original is None else original)
        if not has_eligibility and result.get("reuseKind") in ARTIFACT_REUSE_KINDS:
            raise ValueError("immutable eligibility is required")
        try:
            current = path.lstat()
        except FileNotFoundError as error:
            raise ValueError("request file changed during validation") from error
        if _stamp(metadata) != _stamp(os.fstat(stream.fileno())) or _stamp(metadata) != _stamp(current):
            raise ValueError("request file changed during validation")
        return result


def create_request(request: ReuseRequest, output: Path, root: Path, *,
                   memory_policy_version: str | None = None) -> gate.JsonObject:
    path = _request_path(output, root, existing=False)
    try:
        path.lstat()
    except FileNotFoundError:
        pass
    else:
        raise FileExistsError(f"request already exists: {path}")
    validated = _request(request)
    with _new_git_query(path.parent) as git_query:
        result = _validate(validated, root.resolve(strict=True), git_query=git_query,
                           memory_policy_version=memory_policy_version)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0), 0o600)
    with os.fdopen(descriptor, "w", encoding="utf8") as stream:
        payload: object = validated
        if result.get("reuseKind") in ARTIFACT_REUSE_KINDS:
            payload = {"schema": ELIGIBILITY_SCHEMA, "request": validated, "result": result}
        json.dump(payload, stream, sort_keys=True, indent=2)
        stream.write("\n")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--measured-dir", required=True)
    parser.add_argument("--measured-manifest-sha256", required=True)
    parser.add_argument("--current-build-dir", required=True)
    parser.add_argument("--current-build-manifest-sha256", required=True)
    parser.add_argument("--current-build-artifact-path", required=True)
    parser.add_argument("--current-build-artifact-sha256", required=True)
    parser.add_argument("--request-output", type=Path, required=True)
    policy_group = parser.add_mutually_exclusive_group()
    for number in (3, 4, 5):
        version = f"html-memory-v{number}"
        policy_group.add_argument(f"--require-memory-policy-v{number}",
                                  action="store_const", const=version, dest="memory_policy_version")
    args = parser.parse_args()
    request: ReuseRequest = {
        "schema": SCHEMA, "measuredDir": args.measured_dir,
        "measuredManifestSha256": args.measured_manifest_sha256,
        "currentBuildDir": args.current_build_dir,
        "currentBuildManifestSha256": args.current_build_manifest_sha256,
        "currentBuildArtifactPath": args.current_build_artifact_path,
        "currentBuildArtifactSha256": args.current_build_artifact_sha256,
    }
    try:
        result = create_request(request, args.request_output, args.repo,
                                memory_policy_version=args.memory_policy_version)
        print(json.dumps(result, sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError) as error:
        print(f"HTML performance source reuse failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
