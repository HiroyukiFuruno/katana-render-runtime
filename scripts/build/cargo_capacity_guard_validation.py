"""KRR capacity guardの物理保持と有限候補の検証。削除の許可は行わない。"""
from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
import os
from pathlib import Path
import stat
from typing import Literal, NoReturn

from cargo_capacity_guard import DEFAULT_FACTOR, _integer, _number, inspect
from cargo_capacity_guard_build_script_ownership import FileStamp, SourcePin, file_stamp, source_pin


@dataclass(frozen=True)
class CapacityInputs:
    root: Path
    target: Path
    code: tuple[Path, ...]
    protected: tuple[Path, ...]
    retained_trees: tuple[Path, ...]
    state: Path
    lock: Path


@dataclass(frozen=True)
class PhysicalPin:
    source: SourcePin
    allocated_bytes: int


@dataclass(frozen=True)
class DirectoryPin:
    path: Path
    stamp: FileStamp


@dataclass(frozen=True)
class TreePin:
    directories: tuple[DirectoryPin, ...]
    files: tuple[PhysicalPin, ...]


@dataclass(frozen=True)
class CapacityPins:
    code: tuple[PhysicalPin, ...]
    protected: tuple[PhysicalPin, ...]
    retained_trees: tuple[TreePin, ...]
    state: PhysicalPin
    lock: PhysicalPin


@dataclass(frozen=True)
class CapacitySnapshot:
    inputs: CapacityInputs
    pins: CapacityPins
    current_bytes: int
    threshold_bytes: int | float


@dataclass(frozen=True)
class PinChange:
    path: Path
    classification: Literal["unexpected", "declared-build-output-metadata", "declared-build-output-content"]


@dataclass(frozen=True)
class PreservationComparison:
    strict_unchanged: bool
    changes: tuple[PinChange, ...]


@dataclass(frozen=True)
class CandidatePins:
    snapshot: CapacitySnapshot
    files: tuple[PhysicalPin, ...]


def _canonical(path: Path, root: Path) -> None:
    if not path.is_absolute() or not path.is_relative_to(root):
        raise RuntimeError("capacity validation path is outside canonical root")
    try:
        if path.is_symlink() or path.resolve(strict=True) != path:
            raise RuntimeError("capacity validation path is not canonical or is a symlink")
    except OSError as error:
        raise RuntimeError("capacity validation path is unavailable") from error


def _directory(path: Path, root: Path) -> DirectoryPin:
    _canonical(path, root)
    value = path.lstat()
    if not stat.S_ISDIR(value.st_mode) or file_stamp(value)[-1] != 0:
        raise RuntimeError("capacity validation requires an unflagged directory")
    return DirectoryPin(path, file_stamp(value))


def _physical(path: Path, root: Path) -> PhysicalPin:
    _canonical(path, root)
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or not hasattr(before, "st_blocks"):
        raise RuntimeError("capacity validation file kind or allocation metric is unsupported")
    pin = source_pin(path)
    after = path.lstat()
    if (file_stamp(before) != pin.stamp or file_stamp(after) != pin.stamp
            or before.st_blocks != after.st_blocks):
        raise RuntimeError("capacity validation file drift while observing")
    return PhysicalPin(pin, after.st_blocks * 512)


def _ordered(paths: tuple[Path, ...]) -> tuple[Path, ...]:
    if len(set(paths)) != len(paths):
        raise RuntimeError("capacity validation duplicate paths")
    return tuple(sorted(paths, key=str))


def _validate_inputs(inputs: CapacityInputs) -> None:
    _directory(inputs.root, inputs.root)
    if inputs.target != inputs.root / "target":
        raise RuntimeError("capacity validation requires the canonical KRR target")
    _directory(inputs.target, inputs.root)
    _ordered(inputs.code + inputs.protected + (inputs.state, inputs.lock))
    for tree in _ordered(inputs.retained_trees):
        _directory(tree, inputs.root)


def _walk_error(error: OSError) -> NoReturn:
    raise RuntimeError("capacity validation retained tree is unavailable") from error


def _tree(path: Path, root: Path) -> TreePin:
    _directory(path, root)
    directories: list[DirectoryPin] = []
    files: list[PhysicalPin] = []
    for here, names, filenames in os.walk(path, followlinks=False, onerror=_walk_error):
        directory = Path(here)
        directories.append(_directory(directory, root))
        for name in names:
            _directory(directory / name, root)
        files.extend(_physical(directory / name, root) for name in sorted(filenames))
    return TreePin(tuple(sorted(directories, key=lambda pin: str(pin.path))),
                   tuple(sorted(files, key=lambda pin: str(pin.source.path))))


def _pins(inputs: CapacityInputs) -> CapacityPins:
    return CapacityPins(
        tuple(_physical(path, inputs.root) for path in _ordered(inputs.code)),
        tuple(_physical(path, inputs.root) for path in _ordered(inputs.protected)),
        tuple(_tree(path, inputs.root) for path in _ordered(inputs.retained_trees)),
        _physical(inputs.state, inputs.root), _physical(inputs.lock, inputs.root),
    )


def capture_snapshot(inputs: CapacityInputs) -> CapacitySnapshot:
    _validate_inputs(inputs)
    anchors = (_directory(inputs.root, inputs.root), _directory(inputs.target, inputs.root))
    before = _pins(inputs)
    observation = inspect(inputs.root, inputs.target, inputs.state, DEFAULT_FACTOR, before.state.source.sha256)
    threshold = observation["threshold_bytes"]
    if threshold is None:
        raise RuntimeError("capacity validation requires an existing fixed threshold")
    current = _integer(observation["current_bytes"], "current capacity bytes")
    fixed = _number(threshold, "fixed capacity threshold")
    if (_pins(inputs) != before
            or (_directory(inputs.root, inputs.root), _directory(inputs.target, inputs.root)) != anchors):
        raise RuntimeError("capacity validation input drift while observing")
    return CapacitySnapshot(inputs, before, current, fixed)


def verify_code_sha256(snapshot: CapacitySnapshot, expected: Mapping[Path, str]) -> None:
    actual = {pin.source.path: pin.source.sha256 for pin in snapshot.pins.code}
    if actual.keys() != expected.keys():
        raise RuntimeError("capacity validation code set differs")
    if actual != dict(expected):
        raise RuntimeError("capacity validation code SHA differs")


def _all_pins(pins: CapacityPins) -> dict[Path, PhysicalPin | DirectoryPin]:
    files = pins.code + pins.protected + (pins.state, pins.lock)
    result: dict[Path, PhysicalPin | DirectoryPin] = {pin.source.path: pin for pin in files}
    for tree in pins.retained_trees:
        result.update({pin.path: pin for pin in tree.directories})
        result.update({pin.source.path: pin for pin in tree.files})
    return result


def compare_snapshots(
    before: CapacitySnapshot, after: CapacitySnapshot, declared_build_outputs: tuple[Path, ...] = (),
) -> PreservationComparison:
    if before.inputs != after.inputs:
        raise RuntimeError("capacity validation inputs differ")
    outputs = set(_ordered(declared_build_outputs))
    if (not outputs.issubset(before.inputs.protected)
            or outputs.intersection(before.inputs.code + (before.inputs.state, before.inputs.lock))
            or any(path.is_relative_to(tree) for path in outputs for tree in before.inputs.retained_trees)):
        raise RuntimeError("capacity validation declared outputs must be independent protected files")
    old = _all_pins(before.pins)
    new = _all_pins(after.pins)
    changes = []
    for path in sorted(old.keys() | new.keys(), key=str):
        previous, current = old.get(path), new.get(path)
        if previous == current:
            continue
        classification: Literal["unexpected", "declared-build-output-metadata", "declared-build-output-content"] = "unexpected"
        if path in outputs and isinstance(previous, PhysicalPin) and isinstance(current, PhysicalPin):
            # build対象の変化を説明しても、元の厳格な保持判定は成功に変えない。
            same_content = (previous.source.sha256 == current.source.sha256
                            and previous.source.stamp[4] == current.source.stamp[4]
                            and previous.allocated_bytes == current.allocated_bytes)
            classification = "declared-build-output-metadata" if same_content else "declared-build-output-content"
        changes.append(PinChange(path, classification))
    return PreservationComparison(not changes, tuple(changes))


def capture_candidates(snapshot: CapacitySnapshot, paths: tuple[Path, ...]) -> CandidatePins:
    inputs = snapshot.inputs
    _validate_inputs(inputs)
    anchors = (_directory(inputs.root, inputs.root), _directory(inputs.target, inputs.root))
    if _pins(inputs) != snapshot.pins:
        raise RuntimeError("capacity validation snapshot drift before candidate observation")
    retained_identities = {pin.source.stamp[:2] for pin in _all_pins(snapshot.pins).values()
                           if isinstance(pin, PhysicalPin)}
    files = []
    for path in _ordered(paths):
        if not path.is_relative_to(inputs.target):
            raise RuntimeError("capacity validation candidate is outside target")
        pin = _physical(path, inputs.root)
        if pin.source.stamp[:2] in retained_identities:
            raise RuntimeError("capacity validation candidate intersects a retained file")
        files.append(pin)
    result = CandidatePins(snapshot, tuple(files))
    if (tuple(_physical(pin.source.path, inputs.root) for pin in result.files) != result.files
            or _pins(inputs) != snapshot.pins
            or (_directory(inputs.root, inputs.root), _directory(inputs.target, inputs.root)) != anchors):
        raise RuntimeError("capacity validation candidate or protection drift while observing")
    return result


def compare_candidates(before: CandidatePins, after: CandidatePins) -> bool:
    if before.snapshot.inputs != after.snapshot.inputs:
        raise RuntimeError("capacity validation candidate inputs differ")
    return before.snapshot.pins == after.snapshot.pins and before.files == after.files
