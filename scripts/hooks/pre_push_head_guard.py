#!/usr/bin/env python3
from __future__ import annotations

import os
import stat
import subprocess
import sys
from pathlib import Path

from verify_push_issue import ContractViolation, parse_push_updates


def _git_head(repository: Path) -> str:
    result = subprocess.run(
        ["git", "--no-replace-objects", "rev-parse", "HEAD"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def _git_bytes(repository: Path, *arguments: str) -> bytes:
    result = subprocess.run(
        ["git", "--no-replace-objects", *arguments],
        cwd=repository,
        check=True,
        capture_output=True,
    )
    return result.stdout


def _supports_worktree_executable_mode(repository: Path) -> bool:
    if os.name != "posix":
        return False
    try:
        configured = _git_bytes(repository, "config", "--bool", "--get", "core.filemode")
    except subprocess.CalledProcessError as error:
        if error.returncode != 1:
            raise
        return True
    return configured.strip() == b"true"


def _assert_checkout_matches_head(repository: Path, reviewed_head: str) -> None:
    tree_records = _git_bytes(
        repository, "ls-tree", "-r", "-z", "--full-tree", reviewed_head
    ).split(b"\0")
    tree: dict[str, tuple[str, str]] = {}
    for record in tree_records:
        if not record:
            continue
        metadata, raw_name = record.split(b"\t", 1)
        mode, object_type, object_id = metadata.decode("ascii").split()
        name = os.fsdecode(raw_name)
        if object_type == "commit":
            raise ContractViolation(f"submoduleは品質確認できません: {name}")
        tree[name] = (mode, object_id)

    index_records = _git_bytes(repository, "ls-files", "--stage", "-z").split(b"\0")
    index: dict[str, tuple[str, str]] = {}
    for record in index_records:
        if not record:
            continue
        metadata, raw_name = record.split(b"\t", 1)
        mode, object_id, stage = metadata.decode("ascii").split()
        name = os.fsdecode(raw_name)
        if stage != "0":
            raise ContractViolation("unmerged index cannot be pushed")
        index[name] = (mode, object_id)

    for option in ("-v", "-f"):
        flags = _git_bytes(repository, "ls-files", option, "-z").split(b"\0")
        if any(record and not record.startswith(b"H ") for record in flags):
            raise ContractViolation("git index flagがHEADと一致しません")

    if index != tree:
        raise ContractViolation("git indexがHEADと一致しません")

    untracked = _git_bytes(
        repository, "ls-files", "--others", "--exclude-standard", "-z"
    ).split(b"\0")
    if any(untracked):
        raise ContractViolation("未追跡ファイルが品質確認対象に含まれます")

    compare_executable_mode = _supports_worktree_executable_mode(repository)
    tracked_contents: dict[str, bytes] = {}
    for name, (expected_mode, object_id) in tree.items():
        path = repository / name
        try:
            actual_mode = path.lstat().st_mode
        except (FileNotFoundError, NotADirectoryError) as error:
            raise ContractViolation(f"HEADと一致しません: {name}") from error

        if expected_mode == "120000":
            if not stat.S_ISLNK(actual_mode):
                raise ContractViolation(f"HEADと一致しません: {name}")
            actual_bytes = os.fsencode(os.readlink(path))
        elif expected_mode in ("100644", "100755"):
            if not stat.S_ISREG(actual_mode):
                raise ContractViolation(f"HEADと一致しません: {name}")
            executable = bool(actual_mode & 0o111)
            if compare_executable_mode and executable != (expected_mode == "100755"):
                raise ContractViolation(f"HEADと一致しません: {name}")
            actual_bytes = path.read_bytes()
        else:
            raise ContractViolation(f"HEADのentry modeを確認できません: {name}")

        tracked_contents[name] = actual_bytes

    object_ids = sorted({object_id for _mode, object_id in tree.values()})
    if object_ids:
        batch = subprocess.run(
            ["git", "--no-replace-objects", "cat-file", "--batch"],
            cwd=repository,
            check=True,
            capture_output=True,
            input=("\n".join(object_ids) + "\n").encode("ascii"),
        ).stdout
        offset = 0
        committed_blobs: dict[str, bytes] = {}
        for expected_id in object_ids:
            newline = batch.find(b"\n", offset)
            if newline < 0:
                raise ContractViolation("HEAD blobを読み取れません")
            object_id, object_type, size_text = batch[offset:newline].decode("ascii").split()
            if object_id != expected_id or object_type != "blob":
                raise ContractViolation("HEAD blobを読み取れません")
            size = int(size_text)
            start = newline + 1
            end = start + size
            if end >= len(batch) or batch[end:end + 1] != b"\n":
                raise ContractViolation("HEAD blobを読み取れません")
            committed_blobs[object_id] = batch[start:end]
            offset = end + 1

        for name, (_mode, object_id) in tree.items():
            if tracked_contents[name] != committed_blobs[object_id]:
                raise ContractViolation(f"HEADと一致しません: {name}")

    current_head = _git_head(repository)
    if current_head != reviewed_head:
        raise ContractViolation(
            "品質確認中にcheckout HEADが変わりました: "
            f"{reviewed_head} -> {current_head}"
        )


def validate_push_head(raw_updates: str, reviewed_head: str, repository: Path) -> None:
    if len(reviewed_head) != 40 or any(
        character not in "0123456789abcdef" for character in reviewed_head
    ):
        raise ContractViolation("review対象HEADの形式が不正です")

    updates = parse_push_updates(raw_updates)
    branch_updates = tuple(
        (local_sha, remote_ref)
        for _local_ref, local_sha, remote_ref, _remote_sha in updates
        if remote_ref.startswith("refs/heads/") and local_sha != "0" * 40
    )
    for local_sha, remote_ref in branch_updates:
        if local_sha.lower() != reviewed_head:
            raise ContractViolation(
                f"push対象 {remote_ref} がreview対象HEADと異なります: "
                f"{local_sha} != {reviewed_head}"
            )
    current_head = _git_head(repository)
    if branch_updates and current_head != reviewed_head:
        raise ContractViolation(
            "品質確認中にcheckout HEADが変わりました: "
            f"{reviewed_head} -> {current_head}"
        )
    if branch_updates:
        _assert_checkout_matches_head(repository, reviewed_head)


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: pre_push_head_guard.py REVIEWED_HEAD", file=sys.stderr)
        return 2
    try:
        validate_push_head(sys.stdin.read(), sys.argv[1], Path.cwd())
    except (ContractViolation, subprocess.CalledProcessError) as error:
        print(f"pre-push HEAD check failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
