"""標準flagsで再利用しないCargo/Sweep陽性cacheの有限所有根拠。"""
from __future__ import annotations

from collections import defaultdict
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
import json
import os
from pathlib import Path
import re

from cargo_capacity_guard_build_script_ownership import SourcePin, file_stamp, source_pin

FileRows = tuple[tuple[Path, os.stat_result], ...]
FlagGroup = tuple[FileRows, tuple[Path, ...]]


@dataclass(frozen=True)
class DifferentFlagsAnchor:
    fingerprint: Path
    crate: str
    role: str


@dataclass(frozen=True)
class DifferentFlagsOwnership:
    target: Path
    anchors: Mapping[Path, DifferentFlagsAnchor]
    cargo_selected: frozenset[Path]
    verify: Callable[[], None]
    standalone_objects: Mapping[Path, DifferentFlagsAnchor] = field(default_factory=dict)
    absent_paths: tuple[Path, ...] = ()


def create_different_flags_ownership(
    root: Path, target: Path, metadata: dict[str, object],
    cargo_selected: frozenset[Path], sweep_selected: frozenset[Path],
    input_check: Callable[[], None],
    obsolete_selected: frozenset[Path] = frozenset(),
    *, include_standard_metadata: bool = False,
) -> DifferentFlagsOwnership:
    from cargo_capacity_guard import _array, _object, _text

    input_check()
    if os.environ.get("RUSTFLAGS") != "-D warnings" or target != root / "target":
        raise RuntimeError("different flags ownership requires the standard host build")
    bindings: defaultdict[tuple[str, str], list[tuple[Path, Path, bool]]] = defaultdict(list)
    for item in _array(metadata.get("packages"), "different flags packages"):
        package = _object(item, "different flags package")
        name = _text(package.get("name"), "different flags package name")
        manifest = Path(_text(package.get("manifest_path"), "different flags manifest"))
        for value in _array(package.get("targets"), "different flags targets"):
            info = _object(value, "different flags target")
            kinds = {_text(kind, "different flags target kind")
                     for kind in _array(info.get("kind"), "different flags target kinds")}
            if not kinds or not kinds.issubset({"lib", "rlib", "proc-macro"}):
                continue
            crate = _text(info.get("name"), "different flags target name").replace("-", "_")
            source = Path(_text(info.get("src_path"), "different flags target source"))
            bindings[name, crate].append((manifest, source, kinds == {"proc-macro"}))

    fingerprints: list[tuple[SourcePin, str, str, dict[str, object]]] = []
    for path in sorted(cargo_selected):
        parts = path.relative_to(target).parts
        if len(parts) != 4 or parts[:2] != ("debug", ".fingerprint"):
            continue
        match = re.fullmatch(r"(.+)-([a-f0-9]{16})", parts[2])
        library = re.fullmatch(r"lib-([A-Za-z0-9_]+)\.json", parts[3])
        if match is None or library is None or (match[1], library[1]) not in bindings:
            continue
        pin = source_pin(path)
        raw = _object(json.loads(path.read_text()), "different flags fingerprint")
        if file_stamp(path.lstat()) != pin.stamp:
            raise RuntimeError("different flags fingerprint changed while reading")
        if (type(raw.get("rustc")) is not int or type(raw.get("compile_kind")) is not int
                or raw.get("compile_kind") != 0
                or raw.get("rustflags") not in ([], ["-D", "warnings"])):
            continue
        fingerprints.append((pin, match[1], library[1], raw))
    standard_rustc = {raw["rustc"] for _, _, _, raw in fingerprints
                      if raw["rustflags"] == ["-D", "warnings"]}
    if len(standard_rustc) != 1:
        raise RuntimeError("different flags compiler identity is ambiguous")

    anchors: dict[Path, DifferentFlagsAnchor] = {}
    standalone_objects: dict[Path, DifferentFlagsAnchor] = {}
    standalone_json_inventory: set[Path] = set()
    absent_paths: set[Path] = set()
    pins: dict[Path, SourcePin] = {}
    sweep_deps: defaultdict[str, list[Path]] = defaultdict(list)
    for path in sweep_selected:
        if path.parent == target / "debug/deps":
            digest_match = re.search(r"-([a-f0-9]{16})(?:\.|$)", path.name)
            if digest_match is not None:
                sweep_deps[digest_match[1]].append(path)
    for pin, package_name, crate, raw in fingerprints:
        fingerprint = pin.path
        pins[fingerprint] = pin
        if raw["rustc"] not in standard_rustc:
            continue
        definitions = bindings[package_name, crate]
        macro_modes = {mode for _, _, mode in definitions}
        if len(macro_modes) != 1:
            continue
        for manifest, source, _ in definitions:
            pins[manifest] = source_pin(manifest)
            pins[source] = source_pin(source)
        digest = fingerprint.parent.name.rsplit("-", 1)[1]
        library_pattern = re.compile(r"lib" + re.escape(crate + "-" + digest) + r"\.(rlib|rmeta|dylib)")
        object_pattern = re.compile(re.escape(crate + "-" + digest)
            + r"\.(?:[A-Za-z0-9_.]+|[A-Za-z0-9_]+\.[a-f0-9]{16}-cgu\.[0-9]+)\.rcgu\.o")
        for path in sweep_deps[digest]:
            library_match = library_pattern.fullmatch(path.name)
            role = library_match[1] if library_match else "object" if object_pattern.fullmatch(path.name) else ""
            if not role or role == "dylib" and macro_modes != {True} or role == "rlib" and macro_modes != {False}:
                continue
            anchor = DifferentFlagsAnchor(fingerprint, crate, role)
            if (role == "object" and path not in obsolete_selected and raw["rustc"] != 0
                    and set(fingerprint.parent.glob("*.json")) == {fingerprint}):
                if path in standalone_objects and standalone_objects[path] != anchor:
                    raise RuntimeError("standalone object output owner is ambiguous")
                standalone_objects[path] = anchor
                standalone_json_inventory.add(fingerprint)
            standard_metadata = include_standard_metadata and raw["rustflags"] == ["-D", "warnings"] and role == "rmeta"
            if standard_metadata:
                counterparts = (
                    path.with_suffix(".rlib"),
                    path.with_suffix(".dylib"),
                    path.with_suffix(".so"),
                )
                if path.parent.resolve(strict=True) != path.parent:
                    continue
                counterpart_absent = True
                missing_counterparts: list[Path] = []
                for counterpart in counterparts:
                    try:
                        counterpart.lstat()
                    except FileNotFoundError:
                        missing_counterparts.append(counterpart)
                    else:
                        counterpart_absent = False
                        break
                if not counterpart_absent:
                    continue
                absent_paths.update(missing_counterparts)
            if raw["rustflags"] != [] and not standard_metadata:
                continue
            if path in anchors and anchors[path] != anchor:
                raise RuntimeError("different flags output owner is ambiguous")
            anchors[path] = anchor

    def verify() -> None:
        input_check()
        if os.environ.get("RUSTFLAGS") != "-D warnings":
            raise RuntimeError("different flags caller environment changed")
        for pin in pins.values():
            if (pin.path.resolve(strict=True) != pin.path
                    or file_stamp(pin.path.lstat()) != pin.stamp):
                raise RuntimeError("different flags fingerprint or source changed")
        for path in absent_paths:
            if path.parent.resolve(strict=True) != path.parent:
                raise RuntimeError("standard metadata counterpart parent changed")
            try:
                path.lstat()
            except FileNotFoundError:
                continue
            raise RuntimeError("standard metadata linked counterpart appeared")
        if any(set(path.parent.glob("*.json")) != {path} for path in standalone_json_inventory):
            raise RuntimeError("standalone object fingerprint JSON inventory changed")

    verify()
    return DifferentFlagsOwnership(
        target, anchors, cargo_selected, verify, standalone_objects, tuple(sorted(absent_paths))
    )


def different_flags_groups(
    proof: DifferentFlagsOwnership, files: Sequence[tuple[Path, os.stat_result]],
    protected: set[tuple[int, int]], references: Sequence[Path],
) -> tuple[FlagGroup, ...]:
    from cargo_capacity_guard_lock import _rust_session_parts

    proof.verify()
    aliases: defaultdict[tuple[int, int], list[tuple[Path, os.stat_result]]] = defaultdict(list)
    for path, value in files:
        aliases[value.st_dev, value.st_ino].append((path, value))
    groups: list[FlagGroup] = []
    for identity, rows in aliases.items():
        anchors = [proof.anchors[path] for path, _ in rows if path in proof.anchors]
        if (not anchors or identity in protected or len(rows) != rows[0][1].st_nlink
                or len({(anchor.fingerprint, anchor.role) for anchor in anchors}) != 1):
            continue
        anchor = anchors[0]
        sessions: set[Path] = set()
        valid = True
        for path, value in rows:
            if (getattr(value, "st_flags", 0) != 0
                    or any(path == ref or path.is_relative_to(ref) for ref in references)):
                valid = False
                break
            if path in proof.anchors:
                continue
            parts = path.relative_to(proof.target).parts
            if (len(parts) != 5 or parts[:2] != ("debug", "incremental")
                    or re.fullmatch(re.escape(anchor.crate) + r"-[0-9a-z]{13}", parts[2]) is None
                    or path not in proof.cargo_selected or _rust_session_parts(parts[3]) is None
                    or parts[3].endswith("-working")
                    or not (anchor.role == "rmeta" and path.name == "metadata.rmeta"
                            or anchor.role == "object" and path.suffix == ".o")):
                valid = False
                break
            sessions.add(path.parent)
        if valid:
            groups.append((tuple(rows), tuple(sorted(sessions))))
    proof.verify()
    return tuple(groups)
