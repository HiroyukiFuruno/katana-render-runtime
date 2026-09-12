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

REQUIRED_LATEST_RELEASE = "v0.4.19"
REQUIRED_TARGET_RELEASE = "v0.4.20"
# Keep the release intent explicit: the v0.4.18 baseline and fixes tracked by
# the v0.4.20 release issues must all be present in the candidate.
REQUIRED_RELEASE_COMMITS = (
    "02a73d293c04f9635fd3a822ac865bf81d4c8745",
    "8552c63457480379922c7076bcff604b5401ae20",  # #73
    "694ac82a85d555485e46eb46cf882c8db11b2fe5",  # #74
    "007ab829df39ed40bbfcfc19205ad21f3da32fe8",  # #76 implementation
    "91f07699b26567658f76021050ca7ec7b5c10df1",  # #76 regression
    "88d77e45b7b22d7886e2c09cb0ed1432bf237772",  # #76 review repair
    "d34f6ea22dee7bda77201a57470d430cb658382b",  # #76 Windows regression
    "0c67cff9713fca21b1de35315c4ee3f18ae0c0e8",  # #76 observer repair
    "180d1e3ab6ec3ce3182273fed3a19e28a740dfd1",  # #76 font regression
    "65793ab7855881a6b9042d8d400d69f21a09358a",  # #76 Linux coverage
    "28d2a9b7f88497db3ae103ed406cceb1bd0f6ef5",  # #76 review repair
    "ecba4e40d4bc3419c610d3013f8bd723dd2a449d",  # #76 process boundary
    "9c6915c05db47068bb8fc8649279fa0aad9a1dc5",  # #76 Linux regression
    "817f0e889286be30ce5a549539eeb6d2f4d166de",  # #76 file filter
    "bcca7a5fa4bc06ad30b2917b33cd312f771a5828",  # #76 review repair
    "e395b23ac88ced4bcb116d6cc4e468e4376fc392",  # #76 scroll regression
    "fb83a4a7ec423f2a2b41d0f4954b43a496171c68",  # #76 geometry regression
    "72a5f61dda084653406d972c1ee591dc965054ba",  # #76 runtime assets
    "8ec7a153a26750527dcd9a3e17425bc606e576ea",  # #76 latest dependency migration
)
# A squash merge deliberately rewrites commit ancestry.  These refs identify
# the complete v0.4.20 PR tree independently of that ancestry.  Do not shorten
# this range to the initial implementation: each terminal repair is release
# critical and a reconstructed squash must contain every changed path.
REQUIRED_RELEASE_BASE = "0fbf6ee965b3a5f43f609034a59d0c9042353027"
REQUIRED_RELEASE_TREE = "8ec7a153a26750527dcd9a3e17425bc606e576ea"


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
    release_tree: str = REQUIRED_RELEASE_TREE,
) -> bool:
    """Return whether the candidate preserves every path changed by the release."""
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
    paths = tuple(path for path in changed_paths.stdout.split("\0") if path)
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
