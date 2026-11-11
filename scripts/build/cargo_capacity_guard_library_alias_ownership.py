"""Cargoが選ぶ再生成cacheの、物理aliasへ限定した所有根拠。"""
from __future__ import annotations

from collections import defaultdict
from collections.abc import Callable, Sequence
from dataclasses import dataclass, replace
import json
import os
from pathlib import Path
import re
import shutil
import sys

from cargo_capacity_guard_build_script_ownership import SourcePin, file_stamp, source_pin
from cargo_capacity_guard_debug_object_ownership import DebugObjectOwnership, create_debug_object_ownership
from cargo_capacity_guard_selected_cache_ownership import SelectedCacheOwnership, create_selected_cache_ownership
from cargo_capacity_guard_different_flags_ownership import (
    DifferentFlagsOwnership, create_different_flags_ownership, different_flags_groups,
)


@dataclass(frozen=True)
class LibraryAliasOwnership:
    root: Path
    target: Path
    cargo_selected: frozenset[Path]
    sweep_selected: frozenset[Path]
    verify: Callable[[], None]
    cache: SelectedCacheOwnership | None = None
    different_flags: DifferentFlagsOwnership | None = None
    debug_objects: DebugObjectOwnership | None = None
    metadata_only: bool = False
    archives_only: bool = False


@dataclass(frozen=True)
class LibraryAliasGroup:
    files: tuple[tuple[Path, os.stat_result], ...]
    sessions: tuple[Path, ...]


def _combined_verification(cache: SelectedCacheOwnership, alias_verify: Callable[[], None]) -> Callable[[], None]:
    original_cache_check = cache.verify
    def combined() -> None:
        original_cache_check()
        alias_verify()
    return combined


def _macho_noso_records(value: object, expected: Sequence[Path], manifest_pin: dict[str, object]) -> dict[Path, tuple[str, ...]]:
    from cargo_capacity_guard import _array
    import cargo_capacity_guard_native_cli_observation as native_guard

    report = native_guard.exact(value, {"schema", "status", "parser", "manifest", "files"},
                                "compact Mach-O facts")
    native_guard.require(report["schema"] == "kdr-linter/macho-noso-facts/v1"
                         and report["status"] == "verified", "compact Mach-O facts status differs")
    parser = native_guard.exact(report["parser"], {"crate", "version", "checksum", "features"},
                                "compact Mach-O parser pin")
    native_guard.require(parser == {"crate": "object", "version": "0.41.0",
                         "checksum": parser["checksum"], "features": ["read", "std"]}
                         and isinstance(parser["checksum"], str)
                         and len(parser["checksum"]) == 64
                         and all(char in "0123456789abcdef" for char in parser["checksum"]),
                         "compact Mach-O parser pin differs")
    manifest = native_guard.exact(report["manifest"], {"path", "sha256"}, "compact input manifest")
    native_guard.require(manifest == {"path": manifest_pin["path"], "sha256": manifest_pin["sha256"]},
                         "compact input manifest pin differs")
    rows = _array(report["files"], "compact Mach-O files")
    if len(rows) != len(expected):
        raise RuntimeError("compact Mach-O file count differs")
    return _macho_noso_file_rows(rows, expected)


def _macho_noso_file_rows(rows: Sequence[object], expected: Sequence[Path]) -> dict[Path, tuple[str, ...]]:
    from cargo_capacity_guard import _array
    import cargo_capacity_guard_native_cli_observation as native_guard

    result: dict[Path, tuple[str, ...]] = {}
    for row, path in zip(rows, expected, strict=True):
        item = native_guard.exact(row, {"path", "size", "sha256", "format", "nOso"},
                                  "compact Mach-O file")
        native_guard.require(item["path"] == str(path)
                             and item["format"] in ("MachO32", "MachO64"),
                             "compact Mach-O file identity differs")
        pin = source_pin(path)
        native_guard.require(item["size"] == pin.stamp[4] and item["sha256"] == pin.sha256,
                             "compact Mach-O input pin differs")
        symbols = _array(item["nOso"], "compact N_OSO records")
        result[path] = _macho_noso_names(symbols)
    return result


def _macho_noso_names(symbols: Sequence[object]) -> tuple[str, ...]:
    from cargo_capacity_guard import _integer, _text
    import cargo_capacity_guard_native_cli_observation as native_guard

    names: list[str] = []
    indices: set[int] = set()
    for value in symbols:
        row = native_guard.exact(value, {"symbolIndex", "name"}, "compact N_OSO record")
        index = _integer(row["symbolIndex"], "N_OSO symbol index")
        name = _text(row["name"], "N_OSO symbol name")
        if index in indices:
            raise RuntimeError("compact N_OSO symbol index repeats")
        indices.add(index)
        names.append(name)
    return tuple(names)


def _make_compact_noso_query(
    root: Path,
    logs: Path,
    environment: dict[str, str],
) -> tuple[Callable[[Sequence[Path]], dict[Path, tuple[str, ...]]], tuple[SourcePin, ...]]:
    import cargo_capacity_guard_native_cli_observation as native_guard
    from produce_html_performance_evidence import command

    bun = shutil.which("bun")
    rtk = shutil.which("rtk")
    receipt = root / "tmp/quality/rust-facts-host/current/receipt.json"
    runner = root / "scripts/runtime-bundles/runtime-rust-facts-host.ts"
    if bun is None or rtk is None:
        raise RuntimeError("compact Mach-O facts host tools are unavailable")
    pins = tuple(source_pin(Path(item).resolve(strict=True)) for item in (bun, rtk, receipt, runner))
    bun_path, rtk_path, receipt_path, runner_path = (pin.path for pin in pins)

    def execute(paths: Sequence[Path]) -> dict[Path, tuple[str, ...]]:
        if not paths:
            return {}
        rows = [_macho_noso_input_row(path) for path in paths]
        logs.mkdir(parents=True, exist_ok=True)
        manifest_path = logs / "macho-noso-input.json"
        manifest = {"schema": "krr-macho-noso-input/v1", "files": rows}
        manifest_pin = native_guard.exclusive_json(manifest_path, manifest)
        argv = [str(rtk_path), "proxy", str(bun_path), str(runner_path), "run", str(root),
                "macho-noso", str(manifest_path), str(receipt_path)]
        output = command(argv, text=True, cwd=root, env=environment, role="metadata", log_directory=logs)
        report = native_guard.parse_json(output.encode("utf-8"))
        return _macho_noso_records(report, paths, manifest_pin)

    return execute, pins


def _macho_noso_input_row(path: Path) -> dict[str, object]:
    pin = source_pin(path)
    return {"path": str(pin.path), "size": pin.stamp[4], "sha256": pin.sha256}


def _bind_compact_noso_verification(
    query: Callable[[Sequence[Path]], dict[Path, tuple[str, ...]]],
    verify: Callable[[], None],
) -> Callable[[Sequence[Path]], dict[Path, tuple[str, ...]]]:
    def execute(paths: Sequence[Path]) -> dict[Path, tuple[str, ...]]:
        verify()
        result = query(paths)
        verify()
        return result

    return execute


def classifier_paths(raw: str, target: Path, *, sweep: bool) -> frozenset[Path]:
    paths = []
    for line in raw.splitlines():
        if sweep:
            if "Would remove:" not in line:
                continue
            prefix = "[DEBUG] Would remove: "
            if not line.startswith(prefix):
                raise RuntimeError("library alias Sweep output framing differs")
            value: object = json.loads(line[len(prefix):])
            if not isinstance(value, str):
                raise RuntimeError("library alias Sweep path is not a string")
        else:
            value = line
        path = Path(value)
        if (not path.is_absolute() or not path.is_relative_to(target)
                or path.resolve(strict=True) != path or path.is_symlink()):
            raise RuntimeError("library alias classifier path is not canonical")
        paths.append(path)
    if len(paths) != len(set(paths)):
        raise RuntimeError("library alias classifier repeats a path")
    return frozenset(paths)


def _checked_sweep_paths(raw: str, target: Path, toolchain: str | None = None) -> frozenset[Path]:
    lines = raw.splitlines()
    operation = "remove_not_built_with" if toolchain is not None else "remove_older_than"
    root = f"[DEBUG] cleaning: {json.dumps(str(target))} with {operation}"
    profile = f"[DEBUG] cleaning: {json.dumps(str(target / 'debug'))} with remove_not_built_with_in_a_profile"
    banner = f"[INFO] Using specified installed toolchains: {json.dumps([toolchain])}"
    if (any("[ERROR]" in line or "[WARN]" in line for line in lines)
            or root not in lines or profile not in lines
            or toolchain is not None and banner not in lines):
        raise RuntimeError("library alias Sweep compiler classification is incomplete")
    return classifier_paths(raw, target, sweep=True)


def create_library_alias_ownership(
    root: Path, target: Path, metadata: dict[str, object], cargo: str, logs: Path,
    *, metadata_only: bool = False, archives_only: bool = False,
    legacy_llvm_diagnostic: bool = False,
) -> LibraryAliasOwnership:
    from cargo_capacity_guard import _array, _object, _selected_input_check, _text, selected_namespace_supported

    if root.resolve() != root or target != root / "target" or target.resolve() != target:
        raise RuntimeError("library alias owner requires the canonical host target")
    if (metadata.get("target_directory") != str(target)
            or metadata.get("build_directory", metadata.get("target_directory")) != str(target)):
        raise RuntimeError("library alias metadata target differs")
    if not selected_namespace_supported(root):
        raise RuntimeError("library alias compiler overrides are unsupported")
    check = _selected_input_check(root, metadata)
    environment = dict(os.environ)
    cargo_path = Path(cargo).resolve(strict=True)
    rustc_path = cargo_path.parent / "rustc"
    if (cargo_path.name != "cargo" or cargo_path.parent.name != "bin"
            or cargo_path.parent.parent.parent.name != "toolchains"):
        raise RuntimeError("library alias requires a canonical Rustup Cargo toolchain")
    if (Path(shutil.which("cargo") or "").resolve() != cargo_path
            or Path(shutil.which("rustc") or "").resolve() != rustc_path):
        raise RuntimeError("library alias child PATH differs from the selected toolchain")
    toolchain = cargo_path.parent.parent.name
    tools = [cargo_path, rustc_path]
    for name in ("cargo-sweep", "rustc", "rustup"):
        resolved = shutil.which(name)
        if resolved is None:
            raise RuntimeError("library alias classifier tool is unavailable")
        tools.append(Path(resolved).resolve(strict=True))
    sweep_path = tools[2]
    pins = tuple(source_pin(path) for path in tools)
    packages = []
    members = _array(metadata.get("workspace_members"), "library alias workspace")
    sources = []
    for item in _array(metadata.get("packages"), "library alias packages"):
        package = _object(item, "library alias package")
        package_name = _text(package.get("name"), "library alias package name")
        if package_name not in ("kdr-linter", "katana-render-runtime") or package.get("id") not in members:
            continue
        manifest = Path(_text(package.get("manifest_path"), "library alias manifest"))
        if manifest != root / "crates" / package_name / "Cargo.toml":
            raise RuntimeError("library alias package manifest differs")
        sources.append(source_pin(manifest))
        for target_info in _array(package.get("targets"), "library alias targets"):
            info = _object(target_info, "library alias target")
            path = Path(_text(info.get("src_path"), "library alias source"))
            if not path.is_relative_to(manifest.parent):
                raise RuntimeError("library alias source escapes its package")
            sources.append(source_pin(path))
        packages.append(package_name)
    if sorted(packages) != ["katana-render-runtime", "kdr-linter"]:
        raise RuntimeError("library alias package metadata is incomplete")
    parser_root = Path(__file__).resolve().parents[1] / "release"
    sys.path.insert(0, str(parser_root))
    try:
        from produce_html_performance_evidence import command
    finally:
        sys.path.remove(str(parser_root))
    if Path(command.__code__.co_filename).resolve() != parser_root / "produce_html_performance_evidence.py":
        raise RuntimeError("library alias process owner has a foreign origin")

    compact_query = None
    compact_inputs: tuple[SourcePin, ...] = ()
    if sys.platform == "darwin" and not metadata_only and not legacy_llvm_diagnostic:
        compact_query, compact_inputs = _make_compact_noso_query(root, logs, environment)

    cache: SelectedCacheOwnership | None = None
    def verify() -> None:
        check()
        if (dict(os.environ) != environment
                or any(file_stamp(pin.path.lstat()) != pin.stamp or pin.path.resolve() != pin.path
                       for pin in pins + tuple(sources) + compact_inputs)
                or cache is not None and any(file_stamp(pin.path.lstat()) != pin.stamp
                    for item in cache.owners for pin in (item.manifest, item.source))):
            raise RuntimeError("library alias classifier inputs changed")

    def query(argv: list[str]) -> str:
        verify()
        query_environment = environment
        if argv and argv[0] == str(sweep_path):
            query_environment = {**environment, "CARGO": str(cargo_path)}
        result = command(argv, role="metadata", cwd=root, env=query_environment,
                         log_directory=logs, text=True)
        verify()
        return result

    if compact_query is not None:
        compact_query = _bind_compact_noso_verification(compact_query, verify)

    if query([str(sweep_path), "--version"]).strip() != "cargo-sweep 0.8.0":
        raise RuntimeError("library alias Sweep version differs")
    selected: set[Path] = set()
    for name in sorted(packages):
        selected.update(classifier_paths(query([str(cargo_path), "clean", "--dry-run", "-p", name,
            "-vv", "--manifest-path", str(root / "Cargo.toml")]), target, sweep=False))
    obsolete = _checked_sweep_paths(query([str(sweep_path), "sweep", "--dry-run", "--toolchains",
                                      toolchain, "-vv"]), target, toolchain)
    full = _object(json.loads(query([str(cargo_path), "metadata", "--offline", "--locked", "--all-features",
        "--format-version", "1", "--manifest-path", str(root / "Cargo.toml")])), "library Host graph")
    if full.get("workspace_members") != metadata.get("workspace_members"):
        raise RuntimeError("library alias Host graph workspace differs")
    cache = create_selected_cache_ownership(root, target, full, verify, str(cargo_path), library_alias=True)
    verify()
    combined = _combined_verification(cache, verify)
    package_names = sorted({_text(_object(item, "different flags package").get("name"), "package name")
                            for item in _array(full.get("packages"), "different flags packages")})
    clean_command = [str(cargo_path), "clean", "--dry-run", "-vv", "--manifest-path", str(root / "Cargo.toml")]
    for name in package_names:
        clean_command.extend(["-p", name])
    different_selected = classifier_paths(query(clean_command), target, sweep=False)
    all_sweep = _checked_sweep_paths(query([str(sweep_path), "sweep", "--dry-run", "--time", "0", "-vv"]), target)
    different = create_different_flags_ownership(
        root, target, full, different_selected, all_sweep, verify, obsolete,
        include_standard_metadata=metadata_only,
    )
    different.verify()
    confirmed_obsolete = _checked_sweep_paths(query([str(sweep_path), "sweep", "--dry-run", "--toolchains",
        toolchain, "-vv"]), target, toolchain)
    different.verify()
    if confirmed_obsolete != obsolete:
        raise RuntimeError("library alias Sweep compiler classification changed")
    if metadata_only:
        debug_objects = None
    elif archives_only:
        debug_objects = create_debug_object_ownership(target, query, verify, archives=True,
                                                     compact_query=compact_query)
    else:
        debug_objects = create_debug_object_ownership(target, query, verify, compact_query=compact_query)
    def complete() -> None:
        combined()
        different.verify()
        if debug_objects is not None:
            debug_objects.verify()
    cache = replace(cache, verify=complete)
    return LibraryAliasOwnership(
        root, target, frozenset(selected), obsolete, complete, cache, different, debug_objects,
        metadata_only, archives_only,
    )


def library_alias_groups(
    proof: LibraryAliasOwnership, files: Sequence[tuple[Path, os.stat_result]],
    protected: set[tuple[int, int]], references: Sequence[Path],
    *, needed_bytes: int = 0,
) -> tuple[LibraryAliasGroup, ...]:
    from cargo_capacity_guard_lock import _rust_session_parts

    proof.verify()
    aliases: defaultdict[tuple[int, int], list[tuple[Path, os.stat_result]]] = defaultdict(list)
    for path, value in files:
        aliases[value.st_dev, value.st_ino].append((path, value))
    groups = []
    profile = proof.target / "debug"
    for key, rows in aliases.items():
        if (key in protected or len(rows) != rows[0][1].st_nlink
                or not any(path in proof.cargo_selected for path, _ in rows)):
            continue
        sessions: set[Path] = set()
        roles: set[str] = set()
        anchors: set[Path] = set()
        valid = True
        for path, value in rows:
            parts = path.relative_to(proof.target).parts
            if (getattr(value, "st_flags", 0) != 0
                    or any(path == ref or path.is_relative_to(ref) for ref in references)):
                valid = False
                break
            if len(parts) == 5 and parts[:2] == ("debug", "incremental"):
                if (path not in proof.cargo_selected or _rust_session_parts(parts[3]) is None
                        or parts[3].endswith("-working")):
                    valid = False
                    break
                if path.name == "metadata.rmeta":
                    roles.add("metadata")
                elif path.suffix == ".o":
                    roles.add("object")
                else:
                    valid = False
                    break
                sessions.add(path.parent)
            elif len(parts) == 3 and path.parent == profile / "deps" and path in proof.sweep_selected:
                anchors.add(path)
                if re.fullmatch(r"lib[A-Za-z0-9_]+-[a-f0-9]{16}\.rmeta", path.name):
                    roles.add("metadata")
                elif re.fullmatch(r"[A-Za-z0-9_]+-[a-f0-9]{16}\.[A-Za-z0-9_.]+\.rcgu\.o", path.name):
                    roles.add("object")
                else:
                    valid = False
                    break
            else:
                valid = False
                break
        if valid and sessions and anchors and len(roles) == 1:
            groups.append(LibraryAliasGroup(tuple(rows), tuple(sorted(sessions))))
    proof.verify()
    if proof.different_flags is not None:
        owned_inodes = {(row[1].st_dev, row[1].st_ino) for group in groups for row in group.files}
        for flag_rows, flag_sessions in different_flags_groups(proof.different_flags, files, protected, references):
            if (flag_rows[0][1].st_dev, flag_rows[0][1].st_ino) not in owned_inodes:
                groups.append(LibraryAliasGroup(flag_rows, flag_sessions))
    if proof.metadata_only:
        return tuple(group for group in groups if all(path.suffix == ".rmeta" for path, _ in group.files))
    if proof.archives_only:
        if proof.debug_objects is None or proof.different_flags is None:
            return ()
        archive_groups = tuple(
            group for group in groups
            if group.files
            and all(
                path.suffix == ".rlib"
                and proof.different_flags.anchors.get(path) is not None
                and proof.different_flags.anchors[path].role == "rlib"
                for path, _ in group.files
            )
        )
        if not archive_groups:
            return ()
        archive_consumer_groups = tuple(frozenset(path for path, _ in group.files) for group in archive_groups)
        eligible = proof.debug_objects.capture_archives(files, references, archive_consumer_groups, needed_bytes)
        return tuple(group for group in archive_groups
                     if all(path in eligible for path, _ in group.files))
    if proof.debug_objects is not None:
        object_groups = [group for group in groups if any(path.suffix == ".o" for path, _ in group.files)]
        if proof.different_flags is not None:
            object_proof = replace(proof.different_flags, anchors=proof.different_flags.standalone_objects)
            known_inodes = {(group.files[0][1].st_dev, group.files[0][1].st_ino) for group in object_groups}
            object_groups.extend(LibraryAliasGroup(rows, sessions)
                for rows, sessions in different_flags_groups(object_proof, files, protected, references)
                if (rows[0][1].st_dev, rows[0][1].st_ino) not in known_inodes)
        candidates = frozenset(path for group in object_groups for path, _ in group.files)
        index_names = {"dep-graph.bin", "query-cache.bin"}
        indexes_by_session: defaultdict[Path, set[Path]] = defaultdict(set)
        for path, _ in files:
            if path.name in index_names:
                indexes_by_session[path.parent].add(path)
        consumer_groups = tuple((frozenset(path for path, _ in group.files),
            frozenset(path for session in group.sessions for path in indexes_by_session[session]))
            for group in object_groups)
        eligible = proof.debug_objects.capture(files, references, candidates, needed_bytes, consumer_groups)
        if eligible:
            return tuple(group for group in object_groups if all(path in eligible for path, _ in group.files))
    return tuple(group for group in groups
                 if not any(path.suffix == ".o" for path, _ in group.files))
