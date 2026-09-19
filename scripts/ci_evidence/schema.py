"""Fail-closed schema for GitHub recursive Git-tree input evidence."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence
import unicodedata

EVIDENCE_DIRECTORY = "ci-evidence/"
REGULAR_FILE_MODES = frozenset(("100644", "100755"))
OID_LENGTH = 40


class TreeSchemaError(ValueError):
    """Raised when a Git tree response cannot safely be used as an input digest."""


@dataclass(frozen=True)
class CanonicalTreeEntry:
    """A regular blob that participates in the input-tree digest."""

    path: str
    mode: str
    oid: str

    def as_json(self) -> dict[str, str]:
        return {"mode": self.mode, "oid": self.oid, "path": self.path}


def _fail(message: str) -> None:
    raise TreeSchemaError(message)


def _require_mapping(value: object, label: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        _fail(f"{label} must be an object")
    return value


def _require_string(value: object, label: str) -> str:
    if not isinstance(value, str):
        _fail(f"{label} must be a string")
    try:
        value.encode("utf-8", "strict")
    except UnicodeEncodeError as exc:
        raise TreeSchemaError(f"{label} is not valid UTF-8 text") from exc
    return value


def _validate_path(path: str) -> None:
    if not path:
        _fail("tree entry path must not be empty")
    if "\x00" in path:
        _fail("tree entry path must not contain NUL")
    if path.startswith("/") or path.endswith("/"):
        _fail("tree entry path must be a relative file path")
    parts = path.split("/")
    if any(part in ("", ".", "..") for part in parts):
        _fail("tree entry path must not contain empty, dot, or parent segments")
    if unicodedata.normalize("NFC", path) != path:
        _fail("tree entry path must be NFC normalized")


def _validate_oid(oid: str) -> None:
    if len(oid) != OID_LENGTH:
        _fail(f"tree entry oid must be exactly {OID_LENGTH} lowercase hex characters")
    if any(character not in "0123456789abcdef" for character in oid):
        _fail("tree entry oid must be lowercase hexadecimal")


def _validate_entry(entry: object, index: int) -> tuple[str, str, str]:
    item = _require_mapping(entry, f"tree[{index}]")
    path = _require_string(item.get("path"), f"tree[{index}].path")
    mode = _require_string(item.get("mode"), f"tree[{index}].mode")
    entry_type = _require_string(item.get("type"), f"tree[{index}].type")
    oid = _require_string(item.get("sha"), f"tree[{index}].sha")

    _validate_path(path)
    if entry_type != "blob":
        _fail(f"tree[{index}] must be a blob, got {entry_type!r}")
    if mode not in REGULAR_FILE_MODES:
        _fail(f"tree[{index}] must use a regular-file mode, got {mode!r}")
    _validate_oid(oid)

    size = item.get("size")
    if size is not None and (type(size) is not int or size < 0):
        _fail(f"tree[{index}].size must be a non-negative integer when present")
    return path, mode, oid


def canonical_entries(git_tree: object) -> tuple[CanonicalTreeEntry, ...]:
    """Validate a GitHub recursive-tree response and return sorted digest inputs.

    `ci-evidence/` is the only excluded prefix. Every entry, including an
    excluded one, is validated first so malformed evidence cannot hide input.
    """

    document = _require_mapping(git_tree, "git tree")
    truncated = document.get("truncated")
    if type(truncated) is not bool:
        _fail("git tree truncated must be a boolean")
    if truncated:
        _fail("git tree must not be truncated")

    raw_entries = document.get("tree")
    if not isinstance(raw_entries, Sequence) or isinstance(raw_entries, (str, bytes, bytearray)):
        _fail("git tree tree must be an array")

    seen_paths: set[str] = set()
    included: list[CanonicalTreeEntry] = []
    for index, raw_entry in enumerate(raw_entries):
        path, mode, oid = _validate_entry(raw_entry, index)
        if path in seen_paths:
            _fail(f"git tree has duplicate path {path!r}")
        seen_paths.add(path)
        if not path.startswith(EVIDENCE_DIRECTORY):
            included.append(CanonicalTreeEntry(path=path, mode=mode, oid=oid))

    included.sort(key=lambda entry: entry.path.encode("utf-8"))
    return tuple(included)
