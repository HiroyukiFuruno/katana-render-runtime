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
    parser.add_argument("--repo", default="HiroyukiFuruno/katana-diagram-renderer")
    parser.add_argument("--remote", default="origin")
    return parser.parse_args()


def request_headers() -> dict[str, str]:
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "katana-diagram-renderer-release-target-check",
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


def expected_next(latest: StableVersion | None) -> StableVersion:
    if latest is None:
        return StableVersion(0, 1, 0)
    return StableVersion(latest.major, latest.minor, latest.patch + 1)


def main() -> int:
    args = parse_args()
    target = StableVersion.parse(args.target_version)
    latest = latest_github_release(args.repo) or latest_remote_tag(args.remote)
    expected = expected_next(latest)
    if target != expected:
        latest_text = "no remote release" if latest is None else latest.tag()
        print(
            f"Release target sanity check failed: expected {expected.tag()} after {latest_text}, "
            f"got {target.tag()}.",
            file=sys.stderr,
        )
        return 1
    print(f"Release target sanity check passed: {target.tag()}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
