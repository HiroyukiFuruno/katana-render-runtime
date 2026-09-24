#!/usr/bin/env python3
"""Verify that a requested release version follows the remote release line."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from urllib import error, request

REQUIRED_LATEST_RELEASE = "v0.4.20"
REQUIRED_TARGET_RELEASE = "v0.4.21"
# Keep the release intent explicit: the completed issue closures and governance
# repair must both be present in the candidate.
REQUIRED_RELEASE_COMMITS = (
    "574b9405a6087ecb94d321cb610b65500c270b29",  # #64 governance
    "459eef383cf953f4c97ae5b063f4b72931e4a5ee",  # #78/#80 fixes
)
# squash merge は commit の祖先関係を意図的に書き換える。この不変refは、
# 再構成した squash と比較する v0.4.21 の最終 non-gate release 状態である。
# 最終treeとの比較により、release準備で確定した修正と依存更新を保持する。
REQUIRED_RELEASE_BASE = "fc340a40598e1d14fec9182064da3e7f78a2a5a8"
REQUIRED_RELEASE_TERMINAL = "51d80307f9e63ea5e2d264dd9ae3108eb6f2aa86"
RELEASE_GATE_PATHS = frozenset(
    {
        "scripts/release/verify-release-target.py",
        "scripts/release/verify_release_target_test.py",
    }
)


@dataclass(frozen=True, order=True)
class StableVersion:
    major: int
    minor: int
    patch: int

    @classmethod
    def parse(cls, value: str) -> "StableVersion":
        match = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)", value.strip())
        if match is None:
            raise ValueError(f"expected vX.Y.Z, got {value!r}")
        return cls(*(int(it) for it in match.groups()))

    def tag(self) -> str:
        return f"v{self.major}.{self.minor}.{self.patch}"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-version", required=True)
    parser.add_argument(
        "--latest-version",
        help="Override the latest stable version for deterministic guard tests",
    )
    parser.add_argument("--repo", default="HiroyukiFuruno/katana-render-runtime")
    parser.add_argument("--remote", default="origin")
    parser.add_argument(
        "--head-ref",
        default="HEAD",
        help="Git ref that must contain every release intent commit",
    )
    return parser.parse_args()


def request_headers() -> dict[str, str]:
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "katana-render-runtime-release-target-check",
    }
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    return headers


def latest_github_release(repo: str) -> StableVersion | None:
    api_request = request.Request(
        f"https://api.github.com/repos/{repo}/releases/latest", headers=request_headers()
    )
    try:
        with request.urlopen(api_request, timeout=20) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except error.HTTPError as github_error:
        if github_error.code == 404:
            return None
        raise
    if payload.get("draft") or payload.get("prerelease"):
        return None
    return StableVersion.parse(str(payload["tag_name"]))


def latest_remote_tag(remote: str) -> StableVersion | None:
    result = subprocess.run(
        ["git", "ls-remote", "--tags", remote, "refs/tags/v[0-9]*.[0-9]*.[0-9]*"],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    versions: list[StableVersion] = []
    for line in result.stdout.splitlines():
        tag_ref = line.split(maxsplit=1)[1].removeprefix("refs/tags/")
        tag_name = tag_ref.removesuffix("^{}")
        try:
            versions.append(StableVersion.parse(tag_name))
        except ValueError:
            continue
    return max(versions) if versions else None


def missing_required_commits(
    head_ref: str, required_commits: tuple[str, ...] = REQUIRED_RELEASE_COMMITS
) -> list[str]:
    missing: list[str] = []
    for commit in required_commits:
        result = subprocess.run(
            ["git", "merge-base", "--is-ancestor", commit, head_ref],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if result.returncode != 0:
            missing.append(commit)
    return missing


def release_tree_matches(
    head_ref: str,
    release_base: str = REQUIRED_RELEASE_BASE,
    release_tree: str = REQUIRED_RELEASE_TERMINAL,
) -> bool:
    """Return whether a squash candidate preserves the required release tree.

    Gate implementation changes are excluded so a later gate repair is not
    self-referential. Every other path changed from base to terminal must be
    byte-identical in a candidate whose required commits were squash-rewritten.
    """
    changed_paths = subprocess.run(
        [
            "git",
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            release_base,
            release_tree,
        ],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if changed_paths.returncode != 0:
        return False
    paths = tuple(
        path
        for path in changed_paths.stdout.split("\0")
        if path and path not in RELEASE_GATE_PATHS
    )
    if not paths:
        return False
    result = subprocess.run(
        ["git", "diff", "--quiet", "--no-ext-diff", release_tree, head_ref, "--", *paths],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return result.returncode == 0


def main() -> int:
    args = parse_args()
    target = StableVersion.parse(args.target_version)
    latest = (
        StableVersion.parse(args.latest_version)
        if args.latest_version
        else latest_github_release(args.repo) or latest_remote_tag(args.remote)
    )
    required_latest = StableVersion.parse(REQUIRED_LATEST_RELEASE)
    required_target = StableVersion.parse(REQUIRED_TARGET_RELEASE)
    allowed_latest = {required_latest, required_target}
    if latest not in allowed_latest or target != required_target:
        latest_text = "no remote release" if latest is None else latest.tag()
        print(
            f"Release target sanity check failed: expected {required_target.tag()} after "
            f"{required_latest.tag()} or an idempotent retry of {required_target.tag()}, "
            f"resolved {latest_text} and "
            f"got {target.tag()}.",
            file=sys.stderr,
        )
        return 1
    missing_commits = missing_required_commits(args.head_ref)
    if missing_commits and not release_tree_matches(args.head_ref):
        print(
            "Release target sanity check failed: release branch is missing required "
            "commit(s) and does not reproduce their required release tree: "
            f"{', '.join(missing_commits)}.",
            file=sys.stderr,
        )
        return 1
    print(f"Release target sanity check passed: {target.tag()}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
