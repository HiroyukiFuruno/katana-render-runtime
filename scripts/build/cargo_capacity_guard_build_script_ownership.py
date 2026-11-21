"""Metadata-bound ownership for orphan compiled Cargo build scripts."""
from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import re
import stat

FileStamp = tuple[int, int, int, int, int, int, int, int]


def file_stamp(value: os.stat_result) -> FileStamp:
    return (value.st_dev, value.st_ino, value.st_mode, value.st_nlink, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns, getattr(value, "st_flags", 0))


@dataclass(frozen=True)
class SourcePin:
    path: Path
    stamp: FileStamp
    sha256: str


def source_pin(path: Path) -> SourcePin:
    if not path.is_absolute() or path.resolve() != path:
        raise RuntimeError("build-script source path is not canonical")
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or getattr(before, "st_flags", 0) != 0:
            raise RuntimeError("build-script source kind or flags are unsupported")
        with os.fdopen(os.dup(descriptor), "rb") as handle:
            digest = hashlib.file_digest(handle, "sha256").hexdigest()
        after = os.fstat(descriptor)
        if file_stamp(before) != file_stamp(after) or file_stamp(after) != file_stamp(path.lstat()):
            raise RuntimeError("build-script source changed while reading")
        return SourcePin(path, file_stamp(after), digest)
    finally:
        os.close(descriptor)


@dataclass(frozen=True)
class BuildScriptOwner:
    package_id: str
    package_name: str
    manifest: SourcePin
    source: SourcePin


@dataclass(frozen=True)
class BuildScriptOwnership:
    root: Path
    target: Path
    profile: Path
    owners: tuple[BuildScriptOwner, ...]
    verify: Callable[[], None]


def build_script_ownership(
    root: Path, target: Path, metadata: Mapping[str, object], input_check: Callable[[], None],
) -> BuildScriptOwnership:
    # 既存selectorと同じ完全graph・canonical hostの検証を再利用する。
    from cargo_capacity_guard import _array, _object, _text, selected_package_incremental_prefixes

    input_check()
    selected_package_incremental_prefixes(root, metadata)
    if (target != root / "target" or root.resolve() != root or target.resolve() != target
            or metadata.get("target_directory") != str(target)
            or metadata.get("build_directory", str(target)) != str(target)):
        raise RuntimeError("build-script ownership requires the canonical host target")
    packages = {_text(_object(item, "package")["id"], "package id"): _object(item, "package")
                for item in _array(metadata["packages"], "packages")}
    edges = {_text(_object(item, "node")["id"], "node id"):
             [_text(_object(dep, "dependency")["pkg"], "dependency id")
              for dep in _array(_object(item, "node")["deps"], "dependencies")]
             for item in _array(_object(metadata["resolve"], "resolve")["nodes"], "nodes")}
    pending = [identity for identity, package in packages.items()
               if package.get("manifest_path") == str(root / "crates/kdr-linter/Cargo.toml")]
    closure: set[str] = set()
    while pending:
        identity = pending.pop()
        if identity not in closure:
            closure.add(identity)
            pending.extend(edges[identity])
    owners = []
    for identity in sorted(set(packages) - closure):
        package = packages[identity]
        scripts = [_object(item, "target") for item in _array(package["targets"], "targets")
                   if _object(item, "target").get("kind") == ["custom-build"]]
        if not scripts:
            continue
        name = _text(package["name"], "package name")
        if len(scripts) != 1 or re.fullmatch(r"[A-Za-z_][A-Za-z0-9_-]*", name) is None:
            raise RuntimeError("build-script package ownership is ambiguous")
        manifest = Path(_text(package["manifest_path"], "manifest path"))
        source = Path(_text(scripts[0]["src_path"], "custom-build source"))
        if not source.is_relative_to(manifest.parent):
            raise RuntimeError("build-script source escapes its package")
        owners.append(BuildScriptOwner(identity, name, source_pin(manifest), source_pin(source)))
    pinned = tuple(owners)

    def verify() -> None:
        input_check()
        for owner in pinned:
            if source_pin(owner.manifest.path) != owner.manifest or source_pin(owner.source.path) != owner.source:
                raise RuntimeError("build-script owner source inputs changed")

    verify()
    return BuildScriptOwnership(root, target, target / "debug", pinned, verify)


@dataclass(frozen=True)
class OrphanBuildScript:
    directory: Path
    identity: tuple[int, int, int]
    fingerprint_root_identity: tuple[int, int, int]
    leaves: tuple[tuple[Path, os.stat_result], ...]
    sources: tuple[SourcePin, ...]


def _identity(value: os.stat_result) -> tuple[int, int, int]:
    return value.st_dev, value.st_ino, value.st_mode


def _absent_fingerprint(
    profile_fd: int, name: str, expected: tuple[int, int, int] | None = None,
) -> tuple[int, int, int] | None:
    descriptor = os.open(".fingerprint", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=profile_fd)
    try:
        value = os.fstat(descriptor)
        identity = _identity(value)
        if (value.st_dev != os.fstat(profile_fd).st_dev
                or identity != _identity(os.stat(".fingerprint", dir_fd=profile_fd, follow_symlinks=False))
                or expected is not None and identity != expected):
            raise RuntimeError("build-script fingerprint root changed")
        try:
            os.stat(name, dir_fd=descriptor, follow_symlinks=False)
        except FileNotFoundError as error:
            if identity != _identity(os.stat(".fingerprint", dir_fd=profile_fd, follow_symlinks=False)):
                raise RuntimeError("build-script fingerprint root changed during inspection") from error
            return identity
        return None
    finally:
        os.close(descriptor)


def _dep_info_sources(
    text: str, allowed_targets: set[str], package: Path, source: Path, root: Path,
) -> tuple[Path, ...]:
    if "\\" in text or "\x00" in text:
        raise RuntimeError("unsupported build-script dep-info escaping")
    sources: set[Path] = set()
    targets: set[str] = set()
    for line in text.splitlines():
        if not line:
            continue
        left, separator, right = line.partition(":")
        if not separator or not left or any(char.isspace() for char in left):
            raise RuntimeError("unknown build-script dep-info syntax")
        if right.strip():
            if left not in allowed_targets:
                raise RuntimeError("build-script dep-info names a foreign artifact")
            targets.add(left)
            for name in right.split():
                path = Path(name)
                path = path if path.is_absolute() else root / path
                if not path.is_relative_to(package) or path.resolve() != path:
                    raise RuntimeError("build-script dep-info source is outside its package")
                sources.add(path)
        elif (Path(left) if Path(left).is_absolute() else root / left) not in sources:
            raise RuntimeError("unknown build-script dep-info phony source")
    if targets != allowed_targets or source not in sources:
        raise RuntimeError("build-script dep-info does not prove its owner source")
    return tuple(sorted(sources))


def orphan_build_scripts(
    proof: BuildScriptOwnership, profile_fd: int, build_fd: int, device: int,
    protected: set[tuple[int, int]],
) -> tuple[OrphanBuildScript, ...]:
    proof.verify()
    if (_identity(os.fstat(profile_fd)) != _identity(proof.profile.lstat())
            or os.fstat(build_fd).st_dev != device
            or getattr(os.fstat(profile_fd), "st_flags", 0) != 0
            or getattr(os.fstat(build_fd), "st_flags", 0) != 0):
        raise RuntimeError("build-script profile or device changed")
    names = sorted(os.listdir(build_fd))
    fingerprint_identity = _identity(os.stat(".fingerprint", dir_fd=profile_fd, follow_symlinks=False))
    result = []
    for name in names:
        match = re.fullmatch(r"(.+)-([0-9a-f]{16})", name)
        if match is None:
            continue
        owners = [owner for owner in proof.owners if owner.package_name == match.group(1)]
        if not owners:
            continue
        fp_identity = _absent_fingerprint(profile_fd, name, fingerprint_identity)
        if fp_identity is None:
            continue
        descriptor = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=build_fd)
        try:
            directory = proof.profile / "build" / name
            value = os.fstat(descriptor)
            if (value.st_dev != device or getattr(value, "st_flags", 0) != 0
                    or _identity(value) != _identity(os.stat(name, dir_fd=build_fd, follow_symlinks=False))
                    or not os.access(directory, os.W_OK | os.X_OK, effective_ids=True)):
                raise RuntimeError("compiled build-script directory changed")
            binary = f"build_script_build-{match.group(2)}"
            expected = ["build-script-build", binary, binary + ".d"]
            if sorted(os.listdir(descriptor)) != sorted(expected):
                continue
            leaves = []
            for leaf in expected:
                item = os.stat(leaf, dir_fd=descriptor, follow_symlinks=False)
                if (not stat.S_ISREG(item.st_mode) or item.st_dev != device or item.st_nlink != 1
                        or getattr(item, "st_flags", 0) != 0 or (item.st_dev, item.st_ino) in protected):
                    break
                leaves.append((directory / leaf, item))
            if len(leaves) != len(expected):
                continue
            dep_pin = source_pin(directory / (binary + ".d"))
            if dep_pin.stamp != file_stamp(leaves[-1][1]):
                raise RuntimeError("build-script dep-info changed")
            bound = []
            for owner in owners:
                try:
                    sources = _dep_info_sources(dep_pin.path.read_text(),
                                                {str(directory / binary), str(dep_pin.path)},
                                                owner.manifest.path.parent, owner.source.path, proof.root)
                except RuntimeError:
                    continue
                bound.append(tuple(source_pin(path) for path in sources))
            if len(bound) != 1:
                continue
            binaries = [source_pin(directory / leaf) for leaf in expected[:2]]
            if (binaries[0].sha256 != binaries[1].sha256
                    or any(pin.stamp != file_stamp(leaves[index][1]) for index, pin in enumerate(binaries))):
                continue
            if sorted(os.listdir(descriptor)) != sorted(expected) or file_stamp(value) != file_stamp(os.fstat(descriptor)):
                raise RuntimeError("compiled build-script namespace changed during selection")
            if _absent_fingerprint(profile_fd, name, fp_identity) is None:
                raise RuntimeError("compiled build-script fingerprint appeared during selection")
            result.append(OrphanBuildScript(directory, _identity(value), fp_identity, tuple(leaves), bound[0]))
        finally:
            os.close(descriptor)
    if sorted(os.listdir(build_fd)) != names:
        raise RuntimeError("build-script root namespace changed")
    proof.verify()
    if getattr(os.fstat(profile_fd), "st_flags", 0) != 0:
        raise RuntimeError("build-script profile flags changed")
    return tuple(result)


def verify_orphan_build_scripts(
    proof: BuildScriptOwnership, profile_fd: int, directories: Mapping[Path, int],
    records: Sequence[OrphanBuildScript], removed: set[Path],
) -> None:
    proof.verify()
    if getattr(os.fstat(profile_fd), "st_flags", 0) != 0:
        raise RuntimeError("build-script profile flags changed before purge")
    for record in records:
        descriptor = directories[record.directory]
        value = os.fstat(descriptor)
        if (_identity(value) != record.identity or getattr(value, "st_flags", 0) != 0
                or getattr(record.directory.parent.lstat(), "st_flags", 0) != 0
                or _identity(record.directory.lstat()) != record.identity):
            raise RuntimeError("compiled build-script directory changed before purge")
        if _absent_fingerprint(profile_fd, record.directory.name, record.fingerprint_root_identity) is None:
            raise RuntimeError("compiled build-script fingerprint appeared before purge")
        expected = sorted(path.name for path, _ in record.leaves if path not in removed)
        if sorted(os.listdir(descriptor)) != expected:
            raise RuntimeError("compiled build-script namespace changed before purge")
        for path, before in record.leaves:
            if path not in removed and file_stamp(os.stat(path.name, dir_fd=descriptor, follow_symlinks=False)) != file_stamp(before):
                raise RuntimeError("compiled build-script leaf changed before purge")
        for pin in record.sources:
            if source_pin(pin.path) != pin:
                raise RuntimeError("build-script dep-info source changed before purge")
