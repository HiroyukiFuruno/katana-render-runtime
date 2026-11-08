"""Maintain private Git-dir anchors for fixed local quality receipts."""

from __future__ import annotations

import hashlib
import json
import os
import re
import secrets
import stat
import subprocess
from pathlib import Path
from typing import cast

import runtime_script_profiles as profiles

SCHEMA = "krr-native-local-coverage-anchor-v1"
DIRECTORY_NAME = "krr-native-coverage-v1"
PRODUCERS = {
    "root": "root.json",
    "root-nextest-retry": "root-nextest-retry.json",
    "performance": "performance.json",
    "runtime-asset-script-other": "runtime-asset-script-other.json",
    **{producer: f"{producer}.json" for producer in profiles.accepted_producers()},
}
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")

Anchor = dict[str, object]


def _digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _git_directory(repo: Path) -> Path | None:
    forbidden = (
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CEILING_DIRECTORIES",
    )
    if any(name in os.environ for name in forbidden):
        return None
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--absolute-git-dir"],
            cwd=repo,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0 or not result.stdout.strip():
            return None
        path = Path(result.stdout.strip())
        if not path.is_absolute():
            return None
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            return None
        if hasattr(os, "geteuid") and metadata.st_uid != os.geteuid():
            return None
        return path
    except (OSError, ValueError):
        return None


def _anchor_directory(repo: Path, create: bool) -> Path | None:
    if os.name != "posix" or not hasattr(os, "geteuid"):
        return None
    git_directory = _git_directory(repo)
    if git_directory is None:
        return None
    directory = git_directory / DIRECTORY_NAME
    try:
        if create:
            directory.mkdir(mode=0o700)
    except FileExistsError:
        pass
    except OSError:
        return None
    try:
        info = directory.lstat()
    except OSError:
        return None
    if (
        stat.S_ISLNK(info.st_mode)
        or not stat.S_ISDIR(info.st_mode)
        or info.st_uid != os.geteuid()
        or stat.S_IMODE(info.st_mode) != 0o700
    ):
        return None
    try:
        if set(os.listdir(directory)) - set(PRODUCERS.values()):
            return None
    except OSError:
        return None
    return directory


def _anchor_path(directory: Path, producer: str) -> Path:
    filename = PRODUCERS.get(producer)
    if filename is None:
        raise ValueError("unknown local coverage producer")
    return directory / filename


def _read(directory: Path, producer: str) -> Anchor | None:
    path = _anchor_path(directory, producer)
    try:
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        with os.fdopen(descriptor, "rb") as source:
            info = os.fstat(source.fileno())
            if (
                not stat.S_ISREG(info.st_mode)
                or info.st_uid != os.geteuid()
                or stat.S_IMODE(info.st_mode) != 0o600
                or info.st_nlink != 1
            ):
                return None
            value = json.loads(source.read())
    except FileNotFoundError:
        return None
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return None
    if not isinstance(value, dict):
        return None
    state = value.get("state")
    if (
        set(value)
        != {"schema", "producer", "identitySha256", "epoch", "state", "receiptSha256"}
        or value.get("schema") != SCHEMA
        or value.get("producer") != producer
        or not isinstance(value.get("identitySha256"), str)
        or SHA256_RE.fullmatch(cast(str, value["identitySha256"])) is None
        or not isinstance(value.get("epoch"), str)
        or len(cast(str, value["epoch"])) != 64
        or any(char not in "0123456789abcdef" for char in cast(str, value["epoch"]))
        or not isinstance(state, str)
        or state not in ("pending", "valid")
        or (state == "pending" and value.get("receiptSha256") is not None)
        or (
            state == "valid"
            and (
                not isinstance(value.get("receiptSha256"), str)
                or SHA256_RE.fullmatch(cast(str, value["receiptSha256"])) is None
            )
        )
    ):
        return None
    return cast(Anchor, value)


def _write(directory: Path, record: Anchor) -> bool:
    path = _anchor_path(directory, cast(str, record.get("producer")))
    try:
        try:
            info = path.lstat()
        except FileNotFoundError:
            info = None
        if info is not None and (
            stat.S_ISLNK(info.st_mode)
            or not stat.S_ISREG(info.st_mode)
            or info.st_uid != os.geteuid()
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_nlink != 1
        ):
            return False
        temporary = directory / f".{path.name}.{secrets.token_hex(16)}.tmp"
        descriptor = os.open(
            temporary,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
        with os.fdopen(descriptor, "wb") as output:
            output.write(
                (
                    json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n"
                ).encode()
            )
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        directory_fd = os.open(directory, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
        return _read(directory, cast(str, record["producer"])) == record
    except OSError:
        return False
    finally:
        if "temporary" in locals():
            try:
                temporary.unlink()
            except FileNotFoundError:
                pass
            except OSError:
                pass


def _pending(producer: str, identity_sha256: str, epoch: str | None = None) -> Anchor:
    return {
        "schema": SCHEMA,
        "producer": producer,
        "identitySha256": identity_sha256,
        "epoch": epoch or secrets.token_hex(32),
        "state": "pending",
        "receiptSha256": None,
    }


def prepare(repo: Path, producer: str, identity_sha256: str) -> str | None:
    if producer not in PRODUCERS or SHA256_RE.fullmatch(identity_sha256) is None:
        raise ValueError("invalid local coverage anchor identity")
    directory = _anchor_directory(repo, create=True)
    if directory is None:
        return None
    current = _read(directory, producer)
    if current is not None and current["identitySha256"] == identity_sha256:
        return cast(str, current["epoch"])
    pending = _pending(producer, identity_sha256)
    if not _write(directory, pending):
        return None
    return cast(str, pending["epoch"])


def read_epoch(repo: Path, producer: str, identity_sha256: str) -> str | None:
    """Read an existing epoch without changing another identity's anchor."""
    if producer not in PRODUCERS or SHA256_RE.fullmatch(identity_sha256) is None:
        raise ValueError("invalid local coverage anchor identity")
    directory = _anchor_directory(repo, create=False)
    if directory is None:
        return None
    current = _read(directory, producer)
    if current is None:
        return None
    if current["identitySha256"] != identity_sha256:
        raise ValueError("local coverage anchor identity differs")
    return cast(str, current["epoch"])


def _begin_existing(repo: Path, producer: str, identity_sha256: str, epoch: str) -> bool:
    directory = _anchor_directory(repo, create=False)
    if directory is None:
        return False
    current = _read(directory, producer)
    if (
        current is None
        or current["identitySha256"] != identity_sha256
        or current["epoch"] != epoch
    ):
        return False
    # 実行開始時に世代を変えると、事前計算済みのMoonキーと次回キーが毎回ずれる。
    return _write(directory, _pending(producer, identity_sha256, epoch))


def begin_runtime_script(
    repo: Path, producer: str, identity_sha256: str, epoch: str
) -> bool:
    """Invalidate an old success without changing the already computed Moon key."""
    if producer not in {
        "runtime-asset-script-other",
        *profiles.accepted_producers(),
    }:
        raise ValueError("invalid runtime-script anchor producer")
    return _begin_existing(repo, producer, identity_sha256, epoch)


def begin_root_collection(repo: Path, identity_sha256: str, epoch: str) -> bool:
    """Invalidate the previous root success while preserving its identity epoch."""
    return begin_coverage_collection(repo, "root", identity_sha256, epoch)


def begin_coverage_collection(
    repo: Path, producer: str, identity_sha256: str, epoch: str
) -> bool:
    """Invalidate an old root or performance success without changing its key epoch."""
    if (
        producer not in {"root", "performance"}
        or SHA256_RE.fullmatch(identity_sha256) is None
    ):
        return False
    return _begin_existing(repo, producer, identity_sha256, epoch)


def commit(
    repo: Path, producer: str, identity_sha256: str, epoch: str, receipt_bytes: bytes
) -> bool:
    directory = _anchor_directory(repo, create=False)
    if directory is None:
        return False
    current = _read(directory, producer)
    if (
        current is None
        or current["identitySha256"] != identity_sha256
        or current["epoch"] != epoch
    ):
        return False
    valid: Anchor = {
        **current,
        "state": "valid",
        "receiptSha256": _digest(receipt_bytes),
    }
    return _write(directory, valid)


def verify(
    repo: Path, producer: str, identity_sha256: str, epoch: str, receipt_bytes: bytes
) -> bool:
    directory = _anchor_directory(repo, create=False)
    if directory is None:
        return False
    current = _read(directory, producer)
    return bool(
        current is not None
        and current["identitySha256"] == identity_sha256
        and current["epoch"] == epoch
        and current["state"] == "valid"
        and current["receiptSha256"] == _digest(receipt_bytes)
    )


def invalidate(repo: Path, producer: str, identity_sha256: str, epoch: str) -> None:
    directory = _anchor_directory(repo, create=False)
    if directory is None:
        return
    current = _read(directory, producer)
    if (
        current is not None
        and current["identitySha256"] == identity_sha256
        and current["epoch"] == epoch
    ):
        _write(directory, _pending(producer, identity_sha256))
