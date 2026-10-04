#!/usr/bin/env python3
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

from verify_push_issue import ContractViolation, parse_push_updates


def _git_head(repository: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


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
