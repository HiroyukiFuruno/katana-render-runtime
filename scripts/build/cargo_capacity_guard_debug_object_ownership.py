"""通常lock内で取得する、Mach-O debug map消費者の有限な保存証明。"""

from __future__ import annotations

from collections import Counter, defaultdict
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
import hashlib
import logging
import os
from pathlib import Path
import re
import shutil
import stat
import sys
from types import MappingProxyType

from cargo_capacity_guard_build_script_ownership import (
    FileStamp,
    SourcePin,
    file_stamp,
    source_pin,
)

FileRows = Sequence[tuple[Path, os.stat_result]]
Query = Callable[[list[str]], str]
ObjectGroup = tuple[frozenset[Path], frozenset[Path]]
ArchiveGroup = frozenset[Path]
ArchiveMember = tuple[Path, str, Path]
ArchiveReference = tuple[Path, Path, str, Path]
PROGRESS_LOGGER_NAME = "krr.capacity.progress"
_PROGRESS_LOGGER = logging.getLogger(PROGRESS_LOGGER_NAME)
_PROGRESS_LOGGER.setLevel(logging.WARNING)
_PROGRESS_LOGGER.propagate = False
_PROGRESS_LOGGER.addHandler(logging.NullHandler())
_PROGRESS_BATCH_SIZE = 4096
_MACHO = {
    b"\xcf\xfa\xed\xfe": "<",
    b"\xce\xfa\xed\xfe": "<",
    b"\xfe\xed\xfa\xcf": ">",
    b"\xfe\xed\xfa\xce": ">",
}
_FAT = {
    b"\xca\xfe\xba\xbe",
    b"\xbe\xba\xfe\xca",
    b"\xca\xfe\xba\xbf",
    b"\xbf\xba\xfe\xca",
}


def _header(path: Path, length: int) -> bytes:
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise RuntimeError("debug consumer input is not a regular file")
        result = os.read(descriptor, length)
        if file_stamp(before) != file_stamp(os.fstat(descriptor)) or file_stamp(
            before
        ) != file_stamp(path.lstat()):
            raise RuntimeError("debug consumer input changed while reading header")
        return result
    finally:
        os.close(descriptor)


def _official_tool_pin(path: Path) -> SourcePin:
    before = path.lstat()
    if getattr(before, "st_flags", 0) == 0:
        return source_pin(path)
    # 公式finderと選択されたLLVMだけ、OSのSIP保護・透過圧縮をflag込みで固定する。
    allowed = getattr(stat, "SF_RESTRICTED", 0) | getattr(stat, "UF_COMPRESSED", 0)
    if (
        path.resolve() != path
        or path.name not in {"xcrun", "dsymutil", "ar"}
        or before.st_uid != 0
        or before.st_nlink != 1
        or before.st_mode & 0o022
        or getattr(before, "st_flags", 0) & ~allowed
    ):
        raise RuntimeError("debug consumer official tool flags are unsupported")
    return SourcePin(
        path,
        file_stamp(before),
        hashlib.sha256(_header(path, before.st_size)).hexdigest(),
    )


def macho_kind(path: Path) -> int | None:
    return _macho_kind(_header(path, 16))


def _macho_kind(header: bytes) -> int | None:
    if header[:4] in _FAT:
        raise RuntimeError("debug consumer universal Mach-O is unsupported")
    endian = _MACHO.get(header[:4])
    if endian is None:
        return None
    if len(header) != 16:
        raise RuntimeError("debug consumer Mach-O header is incomplete")
    return int.from_bytes(header[12:16], "little" if endian == "<" else "big")


def _debug_paths(raw: str) -> tuple[frozenset[Path], frozenset[ArchiveMember]]:
    if not raw.startswith(
        "----------------------------------------------------------------------\nSymbol table for:"
    ):
        raise RuntimeError("debug consumer symbol table framing differs")
    paths: set[Path] = set()
    archives: set[ArchiveMember] = set()
    for line in raw.splitlines():
        if re.search(r"\bN_OSO\b", line) is None:
            continue
        match = re.fullmatch(
            r"\[\s*\d+\]\s+[0-9a-f]+ 66 \(N_OSO\s*\)\s+"
            r"[0-9a-f]+\s+[0-9a-f]+\s+[0-9a-f]+ '([^'\n]+)'",
            line,
        )
        if match is None:
            raise RuntimeError("debug consumer N_OSO record is malformed")
        name = match[1]
        archive = re.fullmatch(r"(.+\.(?:rlib|a))\(([^()]+)\)", name)
        path = Path(archive[1] if archive else name)
        if not path.is_absolute() or ".." in path.parts:
            raise RuntimeError("debug consumer N_OSO path is not absolute")
        if path.resolve() != path:
            raise RuntimeError("debug consumer N_OSO path is an untracked alias")
        # 通常archiveのmemberは内部の実体を消費し、同名の外部objectへ読み替えない。
        if archive is None:
            paths.add(path)
        else:
            archives.add((path, archive[2], Path(name)))
    return frozenset(paths), frozenset(archives)


def debug_object_paths(raw: str) -> frozenset[Path]:
    return _debug_paths(raw)[0]


@dataclass(frozen=True)
class ConsumerSnapshot:
    directories: tuple[tuple[Path, tuple[int, int, int, int], frozenset[str]], ...]
    retained: tuple[tuple[Path, FileStamp], ...]
    objects: tuple[tuple[Path, FileStamp], ...]
    missing: tuple[Path, ...] = ()
    groups: tuple[ObjectGroup, ...] = ()
    indexes: tuple[tuple[Path, FileStamp], ...] = ()
    archives: tuple[tuple[Path, FileStamp], ...] = ()
    archive_groups: tuple[ArchiveGroup, ...] = ()
    archive_references: tuple[ArchiveReference, ...] = ()


class DebugObjectOwnership:
    def __init__(
        self,
        target: Path,
        tool: SourcePin,
        query: Query,
        input_check: Callable[[], None],
        finder: SourcePin | None = None,
        archive_tool: SourcePin | None = None,
    ) -> None:
        self.target = target
        self.tool = tool
        self.query = query
        self.input_check = input_check
        self.finder = finder
        self.archive_tool = archive_tool
        self.snapshot: ConsumerSnapshot | None = None
        self.absent: set[Path] = set()
        self.expected: dict[Path, FileStamp] = {}
        self.expected_aliases: Mapping[tuple[int, int], frozenset[Path]] = (
            MappingProxyType({})
        )
        self.selected: frozenset[Path] | None = None

    def _verify_inputs(self) -> None:
        self.input_check()
        pins = (self.tool,) + (() if self.finder is None else (self.finder,))
        pins += () if self.archive_tool is None else (self.archive_tool,)
        for pin in pins:
            if (
                pin.path.resolve() != pin.path
                or file_stamp(pin.path.lstat()) != pin.stamp
            ):
                raise RuntimeError("debug consumer LLVM tool changed")

    def verify(self) -> None:
        self._verify_inputs()
        if self.snapshot is None:
            return
        for path in self.snapshot.missing:
            try:
                path.lstat()
            except FileNotFoundError:
                continue
            raise RuntimeError("debug consumer previously missing input appeared")
        for path, expected in self.expected.items():
            if path in self.absent:
                try:
                    path.lstat()
                except FileNotFoundError:
                    continue
                raise RuntimeError(
                    "debug consumer admitted object changed or reappeared"
                )
            if file_stamp(path.lstat()) != expected:
                raise RuntimeError(
                    "debug consumer admitted object changed or reappeared"
                )
        for path, expected in self.snapshot.retained:
            if file_stamp(path.lstat()) != expected:
                raise RuntimeError("debug consumer retained input changed")
        retained_paths = {path for path, _ in self.snapshot.retained}
        missing_paths = set(self.snapshot.missing)
        if any(
            archive not in retained_paths
            and archive not in missing_paths
            or archive in missing_paths
            and literal not in missing_paths
            for _, archive, _, literal in self.snapshot.archive_references
        ):
            raise RuntimeError(
                "debug consumer archive reference snapshot is incomplete"
            )
        missing_by_parent: defaultdict[Path, set[str]] = defaultdict(set)
        for missing_path in self.absent:
            missing_by_parent[missing_path.parent].add(missing_path.name)
        for path, identity, names in self.snapshot.directories:
            value = path.lstat()
            if (
                value.st_dev,
                value.st_ino,
                value.st_mode,
                getattr(value, "st_flags", 0),
            ) != identity:
                raise RuntimeError("debug consumer directory changed")
            missing = missing_by_parent.get(path, set())
            if frozenset(os.listdir(path)) != names - missing:
                raise RuntimeError("debug consumer namespace changed")

    def _closed_groups(
        self,
        files: FileRows,
        candidates: frozenset[Path],
        groups: Sequence[ObjectGroup],
    ) -> tuple[ObjectGroup, ...]:
        values = dict(files)
        aliases: defaultdict[tuple[int, int], set[Path]] = defaultdict(set)
        indexes_by_session: defaultdict[Path, set[Path]] = defaultdict(set)
        for path, value in files:
            aliases[value.st_dev, value.st_ino].add(path)
            if path.name in {"dep-graph.bin", "query-cache.bin"}:
                indexes_by_session[path.parent].add(path)
        supplied = bool(groups)
        result = (
            tuple(groups)
            if supplied
            else tuple(
                (frozenset({p}), frozenset())
                for p, s in files
                if p in candidates and s.st_nlink == 1
            )
        )
        seen: set[Path] = set()
        for objects, indexes in result:
            if (
                not objects
                or not objects.issubset(candidates)
                or seen.intersection(objects)
            ):
                raise RuntimeError(
                    "debug consumer object groups overlap or escape ownership"
                )
            if not objects.issubset(values) or not indexes.issubset(values):
                raise RuntimeError("debug consumer object group census is incomplete")
            stamps = {file_stamp(values[p]) for p in objects}
            if len(stamps) != 1:
                raise RuntimeError("debug consumer object group identity differs")
            value = values[next(iter(objects))]
            if (
                aliases[value.st_dev, value.st_ino] != set(objects)
                or len(objects) != value.st_nlink
                or getattr(value, "st_flags", 0) != 0
            ):
                raise RuntimeError("debug consumer object alias closure is incomplete")
            sessions = {
                p.parent for p in objects if p.parent != self.target / "debug/deps"
            }
            expected_indexes = frozenset(
                p for session in sessions for p in indexes_by_session[session]
            )
            if indexes != expected_indexes or any(
                values[p].st_nlink != 1 or getattr(values[p], "st_flags", 0) != 0
                for p in indexes
            ):
                raise RuntimeError("debug consumer object index closure differs")
            seen.update(objects)
        if supplied and seen != set(candidates):
            raise RuntimeError("debug consumer object groups do not cover ownership")
        return result

    def _closed_archive_groups(
        self,
        files: FileRows,
        candidates: frozenset[Path],
        groups: Sequence[ArchiveGroup],
    ) -> tuple[ArchiveGroup, ...]:
        values = dict(files)
        aliases: defaultdict[tuple[int, int], set[Path]] = defaultdict(set)
        for path, value in files:
            aliases[value.st_dev, value.st_ino].add(path)
        result = tuple(groups)
        seen: set[Path] = set()
        for group in result:
            if (
                not group
                or not group.issubset(candidates)
                or seen.intersection(group)
                or any(path.suffix != ".rlib" for path in group)
            ):
                raise RuntimeError(
                    "debug consumer archive groups overlap or escape rlib ownership"
                )
            if not group.issubset(values):
                raise RuntimeError("debug consumer archive group census is incomplete")
            stamps = {file_stamp(values[path]) for path in group}
            if len(stamps) != 1:
                raise RuntimeError("debug consumer archive group identity differs")
            value = values[next(iter(group))]
            if (
                aliases[value.st_dev, value.st_ino] != set(group)
                or len(group) != value.st_nlink
                or getattr(value, "st_flags", 0) != 0
            ):
                raise RuntimeError("debug consumer archive alias closure is incomplete")
            seen.update(group)
        if seen != set(candidates):
            raise RuntimeError("debug consumer archive groups do not cover ownership")
        return result

    def _archive_members(
        self,
        path: Path,
        expected: FileStamp,
        cache: dict[Path, tuple[FileStamp, frozenset[str]]],
    ) -> frozenset[str]:
        if self.archive_tool is None:
            raise RuntimeError("debug consumer archive reader is unavailable")
        cached = cache.get(path)
        if cached is not None:
            if cached[0] != expected:
                raise RuntimeError("debug consumer archive changed before reader reuse")
            return cached[1]
        self._verify_inputs()
        if file_stamp(path.lstat()) != expected:
            raise RuntimeError("debug consumer archive changed before reading members")
        output = self.query([str(self.archive_tool.path), "-t", str(path)])
        members = tuple(output.splitlines())
        if (
            not members
            or any(not member or "\0" in member for member in members)
            or file_stamp(path.lstat()) != expected
        ):
            raise RuntimeError("debug consumer archive member listing is invalid")
        result = frozenset(members)
        cache[path] = expected, result
        self._verify_inputs()
        return result

    def capture(
        self,
        files: FileRows,
        references: Sequence[Path],
        candidates: frozenset[Path],
        needed_bytes: int = 0,
        groups: Sequence[ObjectGroup] = (),
    ) -> frozenset[Path]:
        return self._capture(files, references, candidates, needed_bytes, groups, ())

    def capture_archives(
        self,
        files: FileRows,
        references: Sequence[Path],
        groups: Sequence[ArchiveGroup],
        needed_bytes: int = 0,
    ) -> frozenset[Path]:
        if self.archive_tool is None:
            raise RuntimeError("debug consumer archive reader is unavailable")
        if self.snapshot is not None:
            raise RuntimeError("debug consumer archive snapshot cannot be replayed")
        candidates = frozenset(path for group in groups for path in group)
        return self._capture(files, references, candidates, needed_bytes, (), groups)

    def _capture(
        self,
        files: FileRows,
        references: Sequence[Path],
        candidates: frozenset[Path],
        needed_bytes: int,
        groups: Sequence[ObjectGroup],
        archive_groups: Sequence[ArchiveGroup],
    ) -> frozenset[Path]:
        from cargo_capacity_guard_lock import _fingerprint_files

        if self.snapshot is not None:
            raise RuntimeError("debug consumer snapshot cannot be replayed")
        self.verify()
        if not candidates:
            return frozenset()
        archive_candidates = frozenset(
            path for group in archive_groups for path in group
        )
        object_candidates = candidates - archive_candidates
        groups = self._closed_groups(files, object_candidates, groups)
        archive_groups = self._closed_archive_groups(
            files, archive_candidates, archive_groups
        )
        values = dict(files)
        available = sum(
            values[next(iter(objects))].st_blocks * 512 for objects, _ in groups
        )
        available += sum(
            values[next(iter(archives))].st_blocks * 512 for archives in archive_groups
        )
        if available < needed_bytes:
            return frozenset()
        current, directories = _fingerprint_files(
            self.target, self.target.stat().st_dev
        )
        if [(p, file_stamp(s)) for p, s in current] != [
            (p, file_stamp(s)) for p, s in files
        ]:
            raise RuntimeError("debug consumer input census changed")
        original = {p: file_stamp(s) for p, s in files}
        aliases = Counter((s.st_dev, s.st_ino) for _, s in files)
        closed = {
            (s.st_dev, s.st_ino)
            for _, s in files
            if aliases[(s.st_dev, s.st_ino)] == s.st_nlink
        }
        inode_headers: dict[tuple[int, int], tuple[FileStamp, bytes]] = {}
        path_headers: dict[Path, tuple[FileStamp, bytes]] = {}

        def header_for(path: Path, stamp: FileStamp) -> bytes:
            key = (stamp[0], stamp[1])
            cached = inode_headers.get(key) if key in closed else path_headers.get(path)
            if cached is not None:
                if cached[0] != stamp:
                    raise RuntimeError(
                        "debug consumer input changed before header reuse"
                    )
                return cached[1]
            header = _header(path, 16)
            if file_stamp(path.lstat()) != stamp:
                raise RuntimeError("debug consumer input changed while reading header")
            path_headers[path] = (stamp, header)
            if key in closed:
                inode_headers[key] = (stamp, header)
            return header

        directory_pins = tuple(
            (
                p,
                (s[0][0], s[0][1], s[0][2], getattr(p.lstat(), "st_flags", 0)),
                frozenset(names),
            )
            for p, s, names in directories
        )
        retained: dict[Path, FileStamp] = {}
        endpoints: set[Path] = set()
        kinds: dict[Path, int | None] = {}
        archive_member_cache: dict[Path, tuple[FileStamp, frozenset[str]]] = {}
        progress = _PROGRESS_LOGGER.isEnabledFor(logging.INFO)
        classification_paths = sorted({p for p, _ in files} | set(references))
        classification_total = len(classification_paths)
        classification_processed = 0
        if progress:
            _PROGRESS_LOGGER.info(
                "phase=classify_headers state=start total=%d", classification_total
            )
        for path in classification_paths:
            value = path.lstat()
            if not stat.S_ISREG(value.st_mode) or path.resolve() != path:
                raise RuntimeError("debug consumer reference kind is unsupported")
            before = file_stamp(value)
            if path in original and before != original[path]:
                raise RuntimeError("debug consumer input changed before reading")
            retained[path] = before
            header = header_for(path, before)
            if path.suffix in {".rlib", ".a"}:
                if header[:8] != b"!<arch>\n":
                    raise RuntimeError("debug consumer archive is not a full archive")
                retained[path] = before
            kind = _macho_kind(header)
            kinds[path] = kind
            if kind is not None and kind != 1:
                endpoints.add(path)
                retained[path] = before
            if before != file_stamp(path.lstat()):
                raise RuntimeError("debug consumer input changed while reading")
            if progress:
                classification_processed += 1
                if classification_processed % _PROGRESS_BATCH_SIZE == 0:
                    _PROGRESS_LOGGER.info(
                        "phase=classify_headers state=progress processed=%d total=%d",
                        classification_processed,
                        classification_total,
                    )
        if progress:
            _PROGRESS_LOGGER.info(
                "phase=classify_headers state=complete processed=%d total=%d",
                classification_processed,
                classification_total,
            )
        consumed: set[Path] = set()
        consumed_archives: set[Path] = set()
        missing: set[Path] = set()
        archive_references: set[ArchiveReference] = set()
        query_paths = sorted(endpoints)
        if progress:
            _PROGRESS_LOGGER.info(
                "phase=llvm_queries state=start total=%d", len(query_paths)
            )
        query_completed = 0
        for path in query_paths:
            direct, archives = _debug_paths(
                self.query([str(self.tool.path), "--symtab", str(path)])
            )
            consumed.update(direct)
            for object_path in direct:
                try:
                    stamp = file_stamp(object_path.lstat())
                except FileNotFoundError:
                    missing.add(object_path)
                    continue
                if object_path in retained and stamp != retained[object_path]:
                    raise RuntimeError(
                        "debug consumer referenced object changed between queries"
                    )
                retained[object_path] = stamp
            for archive, member, literal in archives:
                consumed_archives.add(archive)
                archive_references.add((path, archive, member, literal))
                try:
                    before = file_stamp(archive.lstat())
                except FileNotFoundError:
                    if not archive.is_relative_to(self.target):
                        raise RuntimeError(
                            "debug consumer missing archive is outside target"
                        ) from None
                    if (
                        not literal.is_relative_to(self.target)
                        or ".." in literal.parts
                        or literal.resolve() != literal
                    ):
                        raise RuntimeError(
                            "debug consumer missing archive literal is not canonical"
                        ) from None
                    try:
                        literal.lstat()
                    except FileNotFoundError:
                        # LLVMはarchive欠落時にmember名を含むliteral全体をopenするため、両方の不存在を固定する。
                        missing.update((archive, literal))
                        continue
                    raise RuntimeError(
                        "debug consumer missing archive literal fallback exists"
                    ) from None
                if archive in retained and before != retained[archive]:
                    raise RuntimeError(
                        "debug consumer referenced archive changed between queries"
                    )
                if header_for(archive, before)[:8] != b"!<arch>\n":
                    raise RuntimeError(
                        "debug consumer referenced archive is not a full archive"
                    )
                if self.archive_tool is not None:
                    if member not in self._archive_members(
                        archive, before, archive_member_cache
                    ):
                        raise RuntimeError(
                            "debug consumer referenced archive member is missing"
                        )
                if before != file_stamp(archive.lstat()):
                    raise RuntimeError("debug consumer referenced archive changed")
                retained[archive] = before
            if progress:
                query_completed += 1
                if query_completed % 64 == 0:
                    _PROGRESS_LOGGER.info(
                        "phase=llvm_queries state=progress processed=%d total=%d",
                        query_completed,
                        len(query_paths),
                    )
        if progress:
            _PROGRESS_LOGGER.info(
                "phase=llvm_queries state=complete processed=%d total=%d",
                query_completed,
                len(query_paths),
            )
        after_files, after_directories = _fingerprint_files(
            self.target, self.target.stat().st_dev
        )
        if [(p, file_stamp(s)) for p, s in after_files] != [
            (p, file_stamp(s)) for p, s in files
        ] or directories != after_directories:
            raise RuntimeError("debug consumer namespace changed during LLVM query")
        admitted = tuple(
            (objects, indexes)
            for objects, indexes in groups
            if all(
                p not in consumed
                and not any(p == ref or p.is_relative_to(ref) for ref in references)
                and kinds[p] == 1
                for p in objects
            )
            and all(
                p not in consumed
                and not any(p == ref or p.is_relative_to(ref) for ref in references)
                for p in indexes
            )
        )
        admitted_archive_groups: list[ArchiveGroup] = []
        for archive_group in archive_groups:
            safe = True
            for archive in archive_group:
                if header_for(archive, original[archive])[:8] != b"!<arch>\n":
                    raise RuntimeError("debug consumer candidate is not a full archive")
                self._archive_members(archive, original[archive], archive_member_cache)
                if (
                    archive in consumed
                    or archive in consumed_archives
                    or any(
                        archive == ref or archive.is_relative_to(ref)
                        for ref in references
                    )
                ):
                    safe = False
            if safe:
                admitted_archive_groups.append(archive_group)
        admitted_archives = tuple(admitted_archive_groups)
        if progress:
            object_direct_rejections = sum(
                any(path in consumed for path in objects | indexes)
                for objects, indexes in groups
            )
            archive_direct_rejections = sum(
                any(path in consumed or path in consumed_archives for path in group)
                for group in archive_groups
            )
            _PROGRESS_LOGGER.info(
                "phase=consumer_filter state=complete object_groups=%d "
                "admitted_object_groups=%d direct_n_oso_object_groups=%d "
                "archive_groups=%d admitted_archive_groups=%d "
                "n_oso_archive_groups=%d reasons_may_overlap=true",
                len(groups),
                len(admitted),
                object_direct_rejections,
                len(archive_groups),
                len(admitted_archives),
                archive_direct_rejections,
            )
        eligible = frozenset(p for objects, _ in admitted for p in objects)
        eligible_archives = frozenset(
            p for archive_group in admitted_archives for p in archive_group
        )
        eligible_bytes = sum(
            values[next(iter(objects))].st_blocks * 512 for objects, _ in admitted
        )
        eligible_bytes += sum(
            values[next(iter(archive_group))].st_blocks * 512
            for archive_group in admitted_archives
        )
        if (not eligible and not eligible_archives) or eligible_bytes < needed_bytes:
            return frozenset()
        index_paths = frozenset(p for _, indexes in admitted for p in indexes)
        self.snapshot = ConsumerSnapshot(
            directory_pins,
            tuple(
                (p, s)
                for p, s in retained.items()
                if p not in eligible | index_paths | eligible_archives
            ),
            tuple((p, original[p]) for p in sorted(eligible)),
            tuple(sorted(missing)),
            admitted,
            tuple((p, original[p]) for p in sorted(index_paths)),
            tuple((p, original[p]) for p in sorted(eligible_archives)),
            admitted_archives,
            tuple(sorted(archive_references)),
        )
        self.expected = dict(
            self.snapshot.objects + self.snapshot.indexes + self.snapshot.archives
        )
        inode_alias_groups: defaultdict[tuple[int, int], set[Path]] = defaultdict(set)
        for path, stamp in self.expected.items():
            inode_alias_groups[stamp[:2]].add(path)
        self.expected_aliases = MappingProxyType(
            {inode: frozenset(paths) for inode, paths in inode_alias_groups.items()}
        )
        self.verify()
        return eligible | eligible_archives

    def validate_selection(self, paths: frozenset[Path]) -> None:
        self.verify()
        if self.snapshot is None:
            return
        allowed: set[Path] = set()
        for objects, indexes in self.snapshot.groups:
            if paths.intersection(objects):
                if not (objects | indexes).issubset(paths):
                    raise RuntimeError(
                        "debug consumer finite admission requires complete object/index groups"
                    )
                allowed.update(objects | indexes)
        for archives in self.snapshot.archive_groups:
            if paths.intersection(archives):
                if not archives.issubset(paths):
                    raise RuntimeError(
                        "debug consumer finite admission requires complete archive groups"
                    )
                allowed.update(archives)
        if not paths.issubset(allowed):
            raise RuntimeError(
                "debug consumer finite admission requires object-only selection and its exact indexes"
            )
        if self.selected is not None and self.selected != paths:
            raise RuntimeError("debug consumer finite selection changed")
        self.selected = paths

    def record_owned_unlink(
        self, path: Path, transition: FileStamp | None = None
    ) -> None:
        if self.snapshot is None:
            return
        self._verify_inputs()
        if self.selected is None or path not in self.selected or path in self.absent:
            raise RuntimeError(
                "debug consumer unlink is outside exact selected ownership"
            )
        try:
            path.lstat()
        except FileNotFoundError:
            pass
        else:
            raise RuntimeError("debug consumer owned unlink did not remove its path")
        previous = self.expected[path]
        inode = previous[:2]
        inode_aliases = self.expected_aliases.get(inode)
        if (
            inode_aliases is None
            or path not in inode_aliases
            or any(
                alias not in self.expected or self.expected[alias][:2] != inode
                for alias in inode_aliases
            )
        ):
            raise RuntimeError(
                "debug consumer alias index differs from expected ownership"
            )
        remaining = [
            alias
            for alias in inode_aliases
            if alias not in self.absent and alias != path
        ]
        if len(remaining) != previous[3] - 1:
            raise RuntimeError("debug consumer owned unlink alias count differs")
        if bool(remaining) != (transition is not None):
            raise RuntimeError(
                "debug consumer owned unlink transition is missing or unexpected"
            )
        actual: FileStamp | None = None
        for alias in remaining:
            value = file_stamp(alias.lstat())
            if (
                value != transition
                or value[:3] != previous[:3]
                or value[3] != previous[3] - 1
                or value[4:6] != previous[4:6]
                or value[7] != previous[7]
                or value[6] < previous[6]
                or actual is not None
                and value != actual
            ):
                raise RuntimeError("debug consumer alias changed after owned unlink")
            actual = value
        if actual is not None:
            self.expected.update((alias, actual) for alias in remaining)
        self.absent.add(path)


def create_debug_object_ownership(
    target: Path,
    query: Query,
    input_check: Callable[[], None],
    *,
    archives: bool = False,
) -> DebugObjectOwnership | None:
    if (
        sys.platform != "darwin"
        or shutil.which("dsymutil") is None
        or shutil.which("xcrun") is None
    ):
        return None
    finder_path = Path("/usr/bin/xcrun")
    finder = _official_tool_pin(finder_path)
    tool = Path(query([str(finder.path), "--find", "dsymutil"]).strip())
    if (
        not tool.is_absolute()
        or tool.resolve() != tool
        or tool.name != "dsymutil"
        or file_stamp(finder.path.lstat()) != finder.stamp
    ):
        raise RuntimeError("debug consumer LLVM tool is not canonical")
    archive_tool = None
    if archives:
        archive = Path(query([str(finder.path), "--find", "ar"]).strip())
        if (
            not archive.is_absolute()
            or archive.resolve() != archive
            or archive.name != "ar"
            or file_stamp(finder.path.lstat()) != finder.stamp
        ):
            raise RuntimeError("debug consumer archive reader is not canonical")
        archive_tool = _official_tool_pin(archive)
    return DebugObjectOwnership(
        target, _official_tool_pin(tool), query, input_check, finder, archive_tool
    )
