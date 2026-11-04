"""Store a private, integrity-bound bootstrap environment."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat
from contextlib import contextmanager
from collections.abc import Iterator


@contextmanager
def _parent(root: Path, path: Path) -> Iterator[tuple[int, str]]:
    if not getattr(os, "O_NOFOLLOW", 0) or not getattr(os, "O_DIRECTORY", 0):
        raise RuntimeError("private launch environment is unsupported on this platform")
    base = root / "tmp"
    if not path.is_absolute() or not path.is_relative_to(base):
        raise RuntimeError("launch environment path must be under the repository tmp directory")
    parts = path.relative_to(base).parts
    if not parts or any(part in ("", ".", "..") for part in parts):
        raise RuntimeError("invalid launch environment path")
    descriptors: list[int] = []
    try:
        current = os.open(base, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        descriptors.append(current)
        for part in parts[:-1]:
            current = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=current)
            descriptors.append(current)
        yield current, parts[-1]
    finally:
        for descriptor in reversed(descriptors):
            os.close(descriptor)


def capture(root: Path, path: Path, environment: dict[str, str]) -> str:
    payload = json.dumps({"version": 1, "environment": environment}, sort_keys=True,
                         separators=(",", ":"), ensure_ascii=True).encode("utf-8")
    with _parent(root, path) as (parent, name):
        descriptor = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                             0o600, dir_fd=parent)
        with os.fdopen(descriptor, "wb") as output:
            os.fchmod(output.fileno(), 0o600)
            output.write(payload)
    return hashlib.sha256(payload).hexdigest()


def _object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate key")
        result[key] = value
    return result


def replay(root: Path, path: Path, expected_sha256: str) -> dict[str, str]:
    if re.fullmatch(r"[0-9a-f]{64}", expected_sha256) is None:
        raise RuntimeError("invalid launch environment digest")
    with _parent(root, path) as (parent, name):
        descriptor = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent)
        with os.fdopen(descriptor, "rb") as source:
            info = os.fstat(source.fileno())
            if (not stat.S_ISREG(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o600
                    or info.st_uid != os.geteuid() or info.st_nlink != 1 or info.st_size > 4 * 1024 * 1024):
                raise RuntimeError("launch environment file is not private")
            payload = source.read(4 * 1024 * 1024 + 1)
    if hashlib.sha256(payload).hexdigest() != expected_sha256:
        raise RuntimeError("launch environment digest differs")
    try:
        document: object = json.loads(payload, object_pairs_hook=_object)
        if (not isinstance(document, dict) or set(document) != {"version", "environment"}
                or type(document["version"]) is not int or document["version"] != 1
                or not isinstance(document["environment"], dict)):
            raise ValueError("invalid schema")
        environment: dict[str, str] = {}
        for key, value in document["environment"].items():
            if (not isinstance(key, str) or not isinstance(value, str) or not key
                    or "=" in key or "\0" in key or "\0" in value):
                raise ValueError("invalid schema")
            environment[key] = value
        return environment
    except (ValueError, UnicodeDecodeError) as error:
        raise RuntimeError("invalid launch environment document") from error
