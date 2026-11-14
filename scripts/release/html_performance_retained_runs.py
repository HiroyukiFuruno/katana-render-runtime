"""Retention for producer-owned HTML performance runs."""
from __future__ import annotations

import json
import os
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
_RETIREMENT = "reference-retirement.json"


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


def _candidate_dependencies(namespace: Path, candidate: Path) -> set[Path]:
    receipt = gate.read_json(candidate / _RECEIPT)
    completed = gate._array(receipt.get("completeConditionIds"), "completed conditions")
    conditions = receipt.get("conditionIds")
    bundle = gate.Bundle(candidate / "measurement/bundle")
    if (receipt.get("formalAccepted") is True and isinstance(conditions, list)
            and len(conditions) == 84 and len(set(conditions)) == 84 and completed == conditions):
        from html_performance_remote_evidence import validate_producer_verification_receipt

        try:
            validate_producer_verification_receipt(bundle.directory, gate._text(receipt.get("head"), "batch HEAD"))
        except (ValueError, OSError, KeyError, TypeError):
            pass
        else:
            return set()
    dependencies: set[Path] = set()
    for original, (_, checksum) in bundle.files.items():
        path = Path(original)
        if not path.is_relative_to(namespace):
            continue
        relative = path.relative_to(namespace)
        gate.require(len(relative.parts) > 1 and _RUN.fullmatch(relative.parts[0]) is not None,
                     "invalid managed dependency path")
        run = namespace / relative.parts[0]
        if not run.exists() and not run.is_symlink():
            continue
        _directory(run)
        gate.require(_regular(path) and gate.sha256(path) == checksum,
                     "managed dependency missing or changed")
        dependencies.add(run)
    return dependencies


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
    try:
        dependencies = _candidate_dependencies(namespace, candidate) if candidate is not None else set()
    except (ValueError, OSError, KeyError, TypeError):
        # 参照が壊れている場合は安全に削除対象を特定できない。
        return
    for run in runs:
        if run not in (baseline, candidate) and run not in dependencies:
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


def _finish_retirement(root: Path, namespace: Path, active: str) -> None:
    from html_performance_legacy_stdout import retire_bundles

    marker = namespace / _RETIREMENT
    if not _regular(marker):
        return
    pending = gate.read_json(marker)
    replacement_sha = gate._text(pending.get("replacementSha256"), "replacement reference SHA")
    retired = gate._text(pending.get("retiredSha256"), "retired reference SHA")
    gate.digest(replacement_sha)
    gate.digest(retired)
    replacement = Path(gate._text(pending.get("replacement"), "replacement manifest"))
    if replacement_sha != active:
        marker.unlink()
        return
    gate.require(_regular(replacement) and gate.sha256(replacement) == active,
                 "selected replacement reference is missing or changed")
    if retire_bundles(root, active, retired, replacement):
        marker.unlink()


def stage_reference_change(root: Path, replacement: Path, replacement_sha: str) -> None:
    """Record the old SHA before updating history so the next run can resume cleanup."""
    namespace = root / "tmp" / _NAMESPACE
    namespace.mkdir(mode=0o700, exist_ok=True)
    _directory(namespace)
    active = _active_manifest(root)
    _finish_retirement(root, namespace, active)
    marker = namespace / _RETIREMENT
    gate.require(not marker.exists() and not marker.is_symlink(), "previous reference retirement is pending")
    temporary = marker.with_suffix(".tmp")
    temporary.unlink(missing_ok=True)
    try:
        with temporary.open("x", encoding="utf8") as output:
            json.dump({"retiredSha256": active, "replacementSha256": replacement_sha,
                       "replacement": str(replacement)}, output)
            output.flush()
            os.fsync(output.fileno())
        temporary.replace(marker)
    finally:
        temporary.unlink(missing_ok=True)


def reconcile_reference(root: Path) -> str:
    """Remove superseded baselines after the verified replacement is selected."""
    active = _active_manifest(root)
    namespace = root / "tmp" / _NAMESPACE
    if namespace.exists():
        _directory(namespace)
        _finish_retirement(root, namespace, active)
        _prune(namespace, active)
    return active


@contextmanager
def managed_output(root: Path, *, baseline: Path | None = None) -> Iterator[Path]:
    """Called under the producer lock; an interrupted run is swept on the next entry."""
    tmp = root / "tmp"
    tmp.mkdir(exist_ok=True)
    _directory(tmp)
    namespace = tmp / _NAMESPACE
    namespace.mkdir(mode=0o700, exist_ok=True)
    _directory(namespace)
    active = reconcile_reference(root)
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
