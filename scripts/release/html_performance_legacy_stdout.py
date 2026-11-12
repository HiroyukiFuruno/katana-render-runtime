"""Discard obsolete stdout from the old HTML measurement output layouts."""
from __future__ import annotations

import argparse
import json
import os
import stat
from pathlib import Path

import verify_html_performance_evidence as gate

_SCHEMA = "krr-html-performance-release-bundle-v1"


def _physical_file(path: Path) -> bool:
    return path.is_file() and path.resolve(strict=True) == path and not path.is_symlink()


def _legacy_manifests(tmp: Path) -> list[Path]:
    return [
        path for pattern in (
            "html-performance-producer*/bundle/bundle.json",
            "quality/*/bundle/bundle.json",
            "quality/*/measurement/bundle/bundle.json",
            "quality/*/*/measurement/bundle/bundle.json",
        )
        for path in tmp.glob(pattern) if _physical_file(path)
    ]


def _stdout_rows(manifest: Path) -> list[tuple[Path, Path]] | None:
    value = gate.read_json(manifest)
    if value.get("schema") != _SCHEMA or not gate._array(value.get("attempts"), "bundle attempts"):
        return None
    rows = []
    for entry in gate._objects(value.get("files"), "bundle files"):
        relative = Path(gate._text(entry.get("relativePath"), "bundle payload"))
        if relative.name != "stdout.raw":
            continue
        if (len(relative.parts) != 3 or relative.parts[0] != "payload"
                or not relative.parts[1].isascii() or not relative.parts[1].isdigit()):
            raise ValueError("unexpected HTML stdout payload layout")
        original = Path(gate._text(entry.get("originalPath"), "bundle original path"))
        rows.append((manifest.parent / relative, original))
    return rows


def prune(root: Path, active: str, candidate: Path, *, apply: bool) -> tuple[int, int]:
    """Called under the producer locks. The candidate must already be sealed."""
    tmp = root / "tmp"
    manifests = _legacy_manifests(tmp)
    if not manifests:
        return 0, 0
    retained = _stdout_rows(candidate)
    if retained is None:
        raise ValueError("retained candidate is not an HTML measurement")
    protected = {path for pair in retained for path in pair}
    reserved: tuple[Path, ...] = ()
    if (references := os.environ.get("CARGO_CAPACITY_PROTECTED_REFS")) is not None:
        from cargo_capacity_guard import collect_protected_references

        reserved = tuple(Path(path).resolve() for path in collect_protected_references([], references))
    pending: set[Path] = set()
    for manifest in manifests:
        is_active = gate.sha256(manifest) == active
        try:
            rows = _stdout_rows(manifest)
        except ValueError:
            if is_active:
                raise
            continue
        if rows is None:
            continue
        if is_active:
            protected.update(path for pair in rows for path in pair)
        else:
            measurement = manifest.parent.parent
            lifecycle = measurement.with_name(measurement.name + ".lifecycle")
            pending.update(payload for payload, _ in rows)
            if lifecycle.is_dir() and lifecycle.resolve(strict=True) == lifecycle:
                pending.update(lifecycle.rglob("stdout.raw"))
    count = allocated = 0
    for path in sorted(pending - protected):
        if any(path.is_relative_to(entry) for entry in reserved) or not _physical_file(path):
            continue
        info = path.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
            continue
        if apply:
            path.unlink()
        count += 1
        allocated += getattr(info, "st_blocks", 0) * 512
    return count, allocated


def main() -> None:
    import html_performance_retained_runs as retention
    from produce_html_performance_evidence import candidate_build_locks

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    root = args.repo.resolve(strict=True)
    with candidate_build_locks(root):
        active = retention._active_manifest(root)
        candidate = retention._current_candidate(root, active)
        count, allocated = prune(root, active, candidate, apply=args.apply)
    print(json.dumps({"applied": args.apply, "stdoutFiles": count, "allocatedBytes": allocated}))


if __name__ == "__main__":
    main()
