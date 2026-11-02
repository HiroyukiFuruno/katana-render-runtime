"""Validate HTML performance source reuse and create an immutable local request."""
from __future__ import annotations

import argparse
import json
import os
import re
import stat
import sys
from pathlib import Path
from typing import TypedDict

import verify_html_performance_evidence as gate

SCHEMA = "krr-html-performance-source-reuse-request-v1"


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


def _validate(request: ReuseRequest, root: Path) -> gate.JsonObject:
    return gate.verify_source_reuse(
        Path(request["measuredDir"]), root, request["measuredManifestSha256"],
        Path(request["currentBuildDir"]), request["currentBuildManifestSha256"],
        request["currentBuildArtifactPath"], request["currentBuildArtifactSha256"],
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


def verify_reuse_request(request_path: Path, root: Path) -> gate.JsonObject:
    path = _request_path(request_path, root, existing=True)
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(descriptor, "r", encoding="utf8") as stream:
        metadata = os.fstat(stream.fileno())
        current = path.lstat()
        if not stat.S_ISREG(metadata.st_mode) or _stamp(metadata) != _stamp(current):
            raise ValueError("request file changed during open")
        value = json.load(stream, object_pairs_hook=gate.unique_object)
        result = _validate(_request(value), root.resolve(strict=True))
        try:
            current = path.lstat()
        except FileNotFoundError as error:
            raise ValueError("request file changed during validation") from error
        if _stamp(metadata) != _stamp(os.fstat(stream.fileno())) or _stamp(metadata) != _stamp(current):
            raise ValueError("request file changed during validation")
        return result


def create_request(request: ReuseRequest, output: Path, root: Path) -> gate.JsonObject:
    path = _request_path(output, root, existing=False)
    try:
        path.lstat()
    except FileNotFoundError:
        pass
    else:
        raise FileExistsError(f"request already exists: {path}")
    validated = _request(request)
    result = _validate(validated, root.resolve(strict=True))
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0), 0o600)
    with os.fdopen(descriptor, "w", encoding="utf8") as stream:
        json.dump(validated, stream, sort_keys=True, indent=2)
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
        result = create_request(request, args.request_output, args.repo)
        print(json.dumps(result, sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError) as error:
        print(f"HTML performance source reuse failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
