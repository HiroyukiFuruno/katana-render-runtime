"""Materialize an immutable Git source tree for a same-host reference build."""
from __future__ import annotations

import hashlib
import json
import os
import stat
from collections.abc import Callable, Sequence
from pathlib import Path, PurePosixPath

import html_performance_reference as reference
import verify_html_performance_evidence as gate
from html_performance_harness_dependency_closure import verify_harness_closure


def _selection_value(selected: reference.ActiveReference) -> dict[str, object]:
    return {
        "recordId": selected.record_id,
        "sourceCommit": selected.source_head,
        "bundleManifestSha256": selected.bundle_manifest_sha256,
        "verifierStdoutSha256": selected.verifier_stdout_sha256,
        "buildPins": dict(sorted(selected.build_pins.items())),
    }


def _tree_records(root: Path, revision: str,
                  records: gate.SourceRecords,
                  git_query: gate.GitQuery | None) -> list[dict[str, object]]:
    rows = _tree(root, revision, git_query)
    gate.require({name for name, _, _ in rows} == set(records),
                 "historical source record membership differs")
    return [{"path": name, "mode": f"{mode:04o}", "sha256": records[name][0],
             "size": records[name][1]} for name, _, mode in rows]


def create_selected_source_proof(root: Path, destination: Path,
                                 selected: reference.ActiveReference,
                                 controller_source_proof: dict[str, str], *,
                                 git_query: gate.GitQuery | None = None) -> dict[str, object]:
    """Bind a materialized active historical source to the controller proof pin."""
    gate.require(set(controller_source_proof) == {"path", "sha256"},
                 "controller source proof pin fields differ")
    proof_path = Path(controller_source_proof["path"])
    gate.require(proof_path.is_absolute() and proof_path.resolve() == proof_path
                 and proof_path.is_file() and not proof_path.is_symlink()
                 and gate.sha256(proof_path) == controller_source_proof["sha256"],
                 "controller source proof pin differs")
    records = verify_selected_source(root, destination, selected, git_query=git_query)
    tree_records = _tree_records(root, selected.source_head, records, git_query)
    payload: dict[str, object] = {
        "schema": "krr-html-performance-selected-source-materialization-v1",
        "selection": _selection_value(selected),
        "controllerSourceProof": dict(controller_source_proof),
        "materializedRoot": str(destination),
        "sourceRecords": tree_records,
    }
    payload["digest"] = hashlib.sha256(
        json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf8")
    ).hexdigest()
    return payload


def verify_selected_source_proof(root: Path, destination: Path,
                                selected: reference.ActiveReference,
                                controller_source_proof: dict[str, str],
                                proof: dict[str, object], *,
                                git_query: gate.GitQuery | None = None) -> gate.SourceRecords:
    """Recheck selection, controller pin, and exact materialized bytes/modes/inventory."""
    gate.require(set(proof) == {"schema", "selection", "controllerSourceProof", "materializedRoot", "sourceRecords", "digest"}
                 and proof.get("schema") == "krr-html-performance-selected-source-materialization-v1"
                 and proof.get("selection") == _selection_value(selected)
                 and proof.get("controllerSourceProof") == controller_source_proof
                 and proof.get("materializedRoot") == str(destination),
                 "selected source materialization identity differs")
    digest = proof.get("digest")
    unsigned = {key: value for key, value in proof.items() if key != "digest"}
    expected_digest = hashlib.sha256(
        json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode("utf8")
    ).hexdigest()
    gate.require(digest == expected_digest, "selected source materialization digest differs")
    pin_path = Path(controller_source_proof["path"])
    gate.require(pin_path.is_absolute() and pin_path.resolve() == pin_path
                 and pin_path.is_file() and not pin_path.is_symlink()
                 and gate.sha256(pin_path) == controller_source_proof["sha256"],
                 "controller source proof pin differs")
    records = verify_selected_source(root, destination, selected, git_query=git_query)
    gate.require(proof.get("sourceRecords") == _tree_records(root, selected.source_head, records, git_query),
                 "selected source tree proof differs")
    return records


def verify_transported_source_proof(root: Path, selected: reference.ActiveReference,
                                   controller_source_proof: dict[str, str],
                                   proof: dict[str, object], producer_root: Path, *,
                                   file_resolver: Callable[[object], Path],
                                   git_query: gate.GitQuery | None = None) -> gate.SourceRecords:
    """Verify the Git-bound logical tree without requiring the producer's host paths."""
    gate.require(set(proof) == {"schema", "selection", "controllerSourceProof", "materializedRoot", "sourceRecords", "digest"}
                 and proof.get("schema") == "krr-html-performance-selected-source-materialization-v1"
                 and proof.get("selection") == _selection_value(selected)
                 and proof.get("controllerSourceProof") == controller_source_proof,
                 "transported historical source identity differs")
    destination = Path(gate._text(proof.get("materializedRoot"), "historical source root"))
    gate.require(producer_root.is_absolute() and destination.is_absolute()
                 and str(destination) == os.path.normpath(str(destination))
                 and destination.is_relative_to(producer_root / "tmp") and destination != producer_root / "tmp",
                 "transported historical source path differs")
    unsigned = {key: value for key, value in proof.items() if key != "digest"}
    gate.require(proof.get("digest") == hashlib.sha256(
        json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode("utf8")).hexdigest(),
        "transported historical source digest differs")
    gate.require(gate.sha256(file_resolver(controller_source_proof)) == controller_source_proof["sha256"],
                 "transported controller source proof differs")
    head = gate.git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    def query(path: Path, args: Sequence[str]) -> str:
        return gate.git(path, *args, git_query=git_query)
    gate.require(reference.active_reference_at_commit(root, head, query) == selected,
                 "transported historical source is not the active reference")
    gate.git(root, "merge-base", "--is-ancestor", selected.source_head, "HEAD", git_query=git_query)
    records = gate.committed_records(root, selected.source_head, git_query=git_query)
    gate.require(proof.get("sourceRecords") == _tree_records(root, selected.source_head, records, git_query),
                 "transported historical source records differ")
    current = gate.committed_records(root, git_query=git_query)
    lock = gate.HARNESS_ROOT + "Cargo.lock"
    gate.require({k: v for k, v in records.items() if k.startswith(gate.HARNESS_ROOT) and k != lock}
                 == {k: v for k, v in current.items() if k.startswith(gate.HARNESS_ROOT) and k != lock},
                 "transported historical harness differs")
    old_lock = gate.git(root, "show", selected.source_head + ":" + lock, git_query=git_query).encode("utf8")
    verify_harness_closure(old_lock, (root / lock).read_bytes(), selected.build_pins["lockSha256"])
    return records


def _tree(root: Path, revision: str, git_query: gate.GitQuery | None) -> list[tuple[str, str, int]]:
    raw = gate._git_output(root, ("ls-tree", "-r", "-z", revision), git_query=git_query)
    if not isinstance(raw, bytes):
        raise ValueError("source tree must be binary")
    rows: list[tuple[str, str, int]] = []
    seen: set[str] = set()
    for entry in raw.split(b"\0"):
        if not entry:
            continue
        metadata, raw_name = entry.split(b"\t", 1)
        mode, kind, oid = metadata.decode("ascii").split()
        name = os.fsdecode(raw_name)
        path = PurePosixPath(name)
        gate.require(kind == "blob" and mode in {"100644", "100755"}
                     and not path.is_absolute() and bool(path.parts)
                     and all(part not in {".", "..", ".git"} for part in path.parts)
                     and str(path) == name and name not in seen,
                     "unsupported historical source entry")
        seen.add(name)
        rows.append((name, oid, int(mode[-3:], 8)))
    gate.require(bool(rows), "historical source tree is empty")
    return rows


def materialize(root: Path, destination: Path, revision: str, *,
                git_query: gate.GitQuery | None = None) -> gate.SourceRecords:
    """Write every selected commit blob once and verify bytes and Git modes."""
    gate.require(len(revision) == 40 and all(c in "0123456789abcdef" for c in revision),
                 "historical source revision is invalid")
    gate.require(root.is_absolute() and root.resolve() == root, "controller root must be canonical")
    temp_root = root / "tmp"
    gate.require(destination.is_absolute() and destination.parent.resolve() == destination.parent
                 and destination.is_relative_to(temp_root) and destination != temp_root,
                 "historical destination must be a canonical temporary child")
    gate.require(not gate.git(root, "status", "--porcelain=v1", "--untracked-files=all", git_query=git_query),
                 "controller source must be clean")
    gate.git(root, "merge-base", "--is-ancestor", revision, "HEAD", git_query=git_query)
    gate.require(gate.git(root, "rev-parse", "--verify", revision + "^{commit}",
                          git_query=git_query).strip() == revision,
                 "historical source commit differs")
    rows = _tree(root, revision, git_query)
    payload = gate._git_output(root, ("cat-file", "--batch"), git_query=git_query,
                               input_bytes="".join(oid + "\n" for _, oid, _ in rows).encode())
    if not isinstance(payload, bytes):
        raise ValueError("source blobs must be binary")
    destination.mkdir(mode=0o700)
    records: gate.SourceRecords = {}
    offset = 0
    for name, oid, mode in rows:
        end = payload.index(b"\n", offset)
        actual_oid, kind, raw_size = payload[offset:end].decode("ascii").split()
        size = int(raw_size)
        start = end + 1
        data = payload[start:start + size]
        gate.require(actual_oid == oid and kind == "blob" and len(data) == size
                     and payload[start + size:start + size + 1] == b"\n"
                     and not data.startswith(b"version https://git-lfs.github.com/spec/v1\n"),
                     "historical blob differs or requires Git LFS")
        path = destination.joinpath(*PurePosixPath(name).parts)
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("xb") as stream:
            stream.write(data)
        path.chmod(mode)
        gate.require(stat.S_ISREG(path.lstat().st_mode) and not path.is_symlink()
                     and stat.S_IMODE(path.lstat().st_mode) == mode,
                     "historical source mode differs")
        checksum = hashlib.sha256(data).hexdigest()
        gate.require(gate.sha256(path) == checksum, "historical source bytes differ")
        records[name] = (checksum, size)
        offset = start + size + 1
    gate.require(offset == len(payload) and records == gate.committed_records(root, revision, git_query=git_query),
                 "historical source inventory differs")
    verify_materialized(root, destination, revision, git_query=git_query)
    return records


def verify_materialized(root: Path, destination: Path, revision: str, *,
                        git_query: gate.GitQuery | None = None) -> gate.SourceRecords:
    rows = _tree(root, revision, git_query)
    expected = gate.committed_records(root, revision, git_query=git_query)
    gate.require(destination.is_dir() and not destination.is_symlink(), "historical source root differs")
    files: set[str] = set()
    folders: set[str] = set()
    expected_folders = {parent.as_posix() for name in expected
                        for parent in PurePosixPath(name).parents if parent.as_posix() != "."}
    for current, directories, names in os.walk(destination, followlinks=False):
        folder = Path(current)
        for name in directories:
            child = folder / name
            gate.require(not child.is_symlink(), "historical source directory is a symlink")
            folders.add(child.relative_to(destination).as_posix())
        files.update((folder / name).relative_to(destination).as_posix() for name in names)
    gate.require(files == set(expected) == {name for name, _, _ in rows} and folders == expected_folders,
                 "historical source file inventory differs")
    for name, _, mode in rows:
        path = destination.joinpath(*PurePosixPath(name).parts)
        info = path.lstat()
        gate.require(stat.S_ISREG(info.st_mode) and stat.S_IMODE(info.st_mode) == mode
                     and (gate.sha256(path), info.st_size) == expected[name],
                     "historical source file differs")
    return expected


def verify_selected_source(root: Path, destination: Path, selected: reference.ActiveReference, *,
                           git_query: gate.GitQuery | None = None) -> gate.SourceRecords:
    """Verify both the current controller and its selected historical source."""
    gate.require(not gate.git(root, "status", "--porcelain=v1", "--untracked-files=all", git_query=git_query),
                 "controller source must be clean")
    head = gate.git(root, "rev-parse", "HEAD", git_query=git_query).strip()
    def query(path: Path, args: Sequence[str]) -> str:
        return gate.git(path, *args, git_query=git_query)
    gate.require(reference.active_reference_at_commit(root, head, query) == selected,
                 "historical source is not the active reference")
    gate.git(root, "merge-base", "--is-ancestor", selected.source_head, "HEAD", git_query=git_query)
    gate.require(destination.is_absolute() and destination.parent.resolve() == destination.parent
                 and destination.is_relative_to(root / "tmp") and destination != root / "tmp",
                 "historical source root is outside controller temporary directory")
    historical = verify_materialized(root, destination, selected.source_head, git_query=git_query)
    controller = gate.committed_records(root, git_query=git_query)
    lock = gate.HARNESS_ROOT + "Cargo.lock"
    def harness(records: gate.SourceRecords) -> gate.SourceRecords:
        return {path: value for path, value in records.items()
                if path.startswith(gate.HARNESS_ROOT) and path != lock}
    gate.require(harness(historical) == harness(controller) and bool(harness(historical)),
                 "historical harness source differs")
    verify_harness_closure((destination / lock).read_bytes(), (root / lock).read_bytes(),
                           selected.build_pins["lockSha256"])
    return historical
