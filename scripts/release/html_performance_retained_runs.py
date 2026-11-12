"""Retention for producer-owned HTML performance runs."""
from __future__ import annotations

import re
import shutil
import stat
import uuid
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

import verify_html_performance_evidence as gate

_NAMESPACE = "html-performance-retained"
_RUN = re.compile(r"run-[0-9a-f]{32}\Z")
_HISTORY = "docs/performance/html-performance-history.json"
_MANIFEST = "measurement/bundle/bundle.json"
_RECEIPT = "measurement/batch-receipt.json"
_CANDIDATE = "candidate.sha256"


def _directory(path: Path) -> None:
    gate.require(stat.S_ISDIR(path.lstat().st_mode) and not path.is_junction()
                 and path.resolve(strict=True) == path, "retention directory is not physical")


def _active_manifest(root: Path) -> str:
    history = gate.read_json(root / _HISTORY)
    active = gate._text(history.get("activeReferenceRecordId"), "active reference")
    records = [row for row in gate._objects(history.get("records"), "history records") if row.get("id") == active]
    gate.require(len(records) == 1, "active reference is missing or ambiguous")
    checksum = gate._text(records[0].get("bundleManifestSha256"), "active reference manifest SHA")
    gate.digest(checksum)
    return checksum


def _regular(path: Path) -> bool:
    return path.is_file() and not path.is_symlink() and path.resolve(strict=True) == path


def _sealed_candidate(run: Path, checksum: str) -> bool:
    receipt_path = run / _RECEIPT
    if not _regular(receipt_path):
        return False
    try:
        receipt = gate.read_json(receipt_path)
        if receipt.get("schema") not in {"krr-html-performance-batch-receipt-v1",
                                        "krr-html-performance-batch-receipt-v2"} or receipt.get("status") != "closed":
            return False
        completed = gate._array(receipt.get("completeConditionIds"), "completed conditions")
        bundle = gate._object(receipt.get("bundle"), "batch bundle")
    except ValueError:
        return False
    return bool(completed) and bundle == {"path": str(run / _MANIFEST), "sha256": checksum}


def _prune(namespace: Path, active: str, *, newest: Path | None = None) -> None:
    runs = [run for run in namespace.iterdir() if _RUN.fullmatch(run.name)]
    marked: list[Path] = []
    candidates: list[Path] = []
    for run in runs:
        _directory(run)
        manifest = run / _MANIFEST
        if not _regular(manifest):
            continue
        checksum = gate.sha256(manifest)
        marker = run / _CANDIDATE
        if checksum == active:
            marker.unlink(missing_ok=True)
            marked.append(run)
        elif _sealed_candidate(run, checksum):
            if run == newest:
                with marker.open("x", encoding="ascii") as output:
                    output.write(checksum)
            if _regular(marker) and marker.read_bytes() == checksum.encode("ascii"):
                candidates.append(run)
    candidate = newest if newest in candidates else max(
        candidates, key=lambda run: ((run / _RECEIPT).stat().st_mtime_ns, run.name), default=None,
    )
    baseline = min(marked, default=None)
    for run in runs:
        if run not in (baseline, candidate):
            shutil.rmtree(run)


def _register_baseline(namespace: Path, source: Path, active: str) -> None:
    if any(_regular(run / _MANIFEST) and gate.sha256(run / _MANIFEST) == active
           for run in namespace.iterdir() if _RUN.fullmatch(run.name)):
        return
    gate.require(gate.sha256(source / "bundle.json") == active, "baseline manifest is not marked")
    source_bundle = gate.Bundle(source)
    run = namespace / f"run-{uuid.uuid4().hex}"
    bundle = run / "measurement/bundle"
    try:
        bundle.mkdir(parents=True, mode=0o700)
        for relative, (path, _, _) in source_bundle.relative_files.items():
            gate.require(source_bundle.directory / relative == path, "noncanonical baseline payload path")
            destination = bundle / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, destination)
        shutil.copyfile(source / "bundle.json", bundle / "bundle.json")
        gate.require(gate.sha256(bundle / "bundle.json") == active, "baseline manifest changed")
        gate.Bundle(bundle)
    except BaseException:
        if run.is_dir():
            shutil.rmtree(run)
        raise


def _current_candidate(root: Path, active: str) -> Path:
    manifests = []
    for run in (root / "tmp" / _NAMESPACE).iterdir():
        if not _RUN.fullmatch(run.name) or not _regular(run / _MANIFEST):
            continue
        manifest, marker = run / _MANIFEST, run / _CANDIDATE
        checksum = gate.sha256(manifest)
        if (checksum != active and _regular(marker) and marker.read_bytes() == checksum.encode("ascii")
                and _sealed_candidate(run, checksum)):
            manifests.append(manifest)
    if len(manifests) != 1:
        raise ValueError("legacy cleanup requires exactly one retained sealed candidate")
    return manifests[0]


@contextmanager
def managed_output(root: Path, *, baseline: Path | None = None) -> Iterator[Path]:
    """Called under the producer lock; an interrupted run is swept on the next entry."""
    active = _active_manifest(root)
    tmp = root / "tmp"
    tmp.mkdir(exist_ok=True)
    _directory(tmp)
    namespace = tmp / _NAMESPACE
    namespace.mkdir(mode=0o700, exist_ok=True)
    _directory(namespace)
    _prune(namespace, active)
    if baseline is not None:
        _register_baseline(namespace, baseline, active)
    run = namespace / f"run-{uuid.uuid4().hex}"
    run.mkdir(mode=0o700)
    try:
        yield run / "measurement"
    finally:
        _prune(namespace, active, newest=run)
        if _regular(run / _CANDIDATE):
            from html_performance_legacy_stdout import prune

            prune(root, active, run / _MANIFEST, apply=True)
