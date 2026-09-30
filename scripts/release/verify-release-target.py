#!/usr/bin/env python3
"""Verify that a requested release version follows the remote release line."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from urllib import error, request

REQUIRED_LATEST_RELEASE = "v0.4.21"
REQUIRED_TARGET_RELEASE = "v0.4.22"
# 実レビュー対象のdefault baseと、v0.4.22で指定された修正commitを固定する。
# リリース内容そのものは不変manifestでも別途検証するが、manifest一致だけで
# 必須修正commitの祖先性を代替してはならない。
REQUIRED_RELEASE_COMMITS = (
    "9ad18a07358cf28b742cc00c12c0c9f85956d20a",  # PR #99のdefault base
    "1bc497bdfd3b8c4318e9ee1609d147b6925b43e5",  # v0.4.22指定修正
)
# squash merge 後も default history に残る v0.4.21 の merge commit を、
# 変更集合の起点として使う。期待値そのものは commit/tree object ではなく、
# base からの non-gate 差分を表す不変な内容 manifest に固定する。
REQUIRED_RELEASE_BASE = "7f984d16400fe3e097a3380dd7641d78c5852352"
# Gate 修正はこの digest から除外するため、squash 後の gate repair によって
# 自己参照しない。raw diff は path、file mode、base/target blob を含む。
REQUIRED_RELEASE_MANIFEST_SHA256 = "75d2ebfaf697dc4a396dec823b09989a0f103f336e41383607a886bdd3a0acaf"
RELEASE_GATE_PATHS = frozenset(
    {
        "scripts/release/verify-release-target.py",
        "scripts/release/verify_release_target_test.py",
    }
)
REQUIRED_RELEASE_MANIFEST_PATTERN = re.compile(
    r'^REQUIRED_RELEASE_MANIFEST_SHA256 = "[0-9a-f]{64}"$', re.MULTILINE
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
    parser.add_argument(
        "--print-release-manifest",
        action="store_true",
        help="Print the immutable manifest digest for --head-ref without changing the guard",
    )
    parser.add_argument(
        "--update-release-manifest",
        action="store_true",
        help="Explicitly synchronize the checked-in manifest with --head-ref",
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


def release_manifest_sha256(
    head_ref: str,
    release_base: str = REQUIRED_RELEASE_BASE,
) -> str | None:
    """Return the immutable release content manifest for ``head_ref``.

    Gate implementation changes are excluded so a later gate repair does not
    become self-referential. Callers must compare the value with the checked-in
    expected digest; calculating it never accepts a candidate by itself.
    """
    changed_paths = subprocess.run(
        [
            "git",
            "diff",
            "--raw",
            "--abbrev=40",
            "-z",
            "--no-renames",
            release_base,
            head_ref,
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if changed_paths.returncode != 0:
        return None
    records = changed_paths.stdout.split(b"\0")
    if records[-1] or len(records) % 2 != 1:
        return None
    gate_paths = {path.encode("utf-8") for path in RELEASE_GATE_PATHS}
    entries: list[tuple[bytes, bytes]] = []
    for raw_record, path in zip(records[:-1:2], records[1:-1:2]):
        if not raw_record.startswith(b":") or not path:
            return None
        if path not in gate_paths:
            entries.append((path, raw_record))
    if not entries:
        return None
    manifest = hashlib.sha256()
    for path, raw_record in sorted(entries):
        manifest.update(raw_record)
        manifest.update(b"\0")
        manifest.update(path)
        manifest.update(b"\0")
    return manifest.hexdigest()


def release_tree_matches(
    head_ref: str,
    release_base: str = REQUIRED_RELEASE_BASE,
    required_manifest_sha256: str = REQUIRED_RELEASE_MANIFEST_SHA256,
) -> bool:
    """Return whether a squash candidate preserves the required release content."""
    return release_manifest_sha256(head_ref, release_base) == required_manifest_sha256


def rewrite_required_release_manifest(source: str, manifest_sha256: str) -> str:
    """Replace exactly one checked-in digest, rejecting malformed source."""
    if not re.fullmatch(r"[0-9a-f]{64}", manifest_sha256):
        raise ValueError("release manifest digest must be 64 lowercase hexadecimal characters")
    replacement = f'REQUIRED_RELEASE_MANIFEST_SHA256 = "{manifest_sha256}"'
    updated, replacements = REQUIRED_RELEASE_MANIFEST_PATTERN.subn(replacement, source, count=1)
    if replacements != 1:
        raise ValueError("expected exactly one required release manifest declaration")
    return updated


def update_required_release_manifest(head_ref: str) -> str:
    """Synchronize the reviewed guard explicitly after release content changes."""
    manifest_sha256 = release_manifest_sha256(head_ref)
    if manifest_sha256 is None:
        raise ValueError(f"could not calculate release manifest for {head_ref!r}")
    script = Path(__file__).resolve()
    source = script.read_text(encoding="utf-8")
    updated = rewrite_required_release_manifest(source, manifest_sha256)
    if updated != source:
        script.write_text(updated, encoding="utf-8")
    return manifest_sha256


def main() -> int:
    args = parse_args()
    target = StableVersion.parse(args.target_version)
    required_target = StableVersion.parse(REQUIRED_TARGET_RELEASE)
    if args.print_release_manifest:
        manifest_sha256 = release_manifest_sha256(args.head_ref)
        if manifest_sha256 is None:
            print(
                f"Release target manifest failed: could not calculate {args.head_ref!r}.",
                file=sys.stderr,
            )
            return 1
        print(manifest_sha256)
        return 0
    if args.update_release_manifest:
        missing_commits = missing_required_commits(args.head_ref)
        if target != required_target or missing_commits:
            print(
                "Release target manifest update failed: target must be "
                f"{required_target.tag()} and head must contain the required release commit(s).",
                file=sys.stderr,
            )
            return 1
        try:
            manifest_sha256 = update_required_release_manifest(args.head_ref)
        except ValueError as update_error:
            print(f"Release target manifest update failed: {update_error}", file=sys.stderr)
            return 1
        print(f"Release target manifest updated: {manifest_sha256}.")
        return 0
    latest = (
        StableVersion.parse(args.latest_version)
        if args.latest_version
        else latest_github_release(args.repo) or latest_remote_tag(args.remote)
    )
    required_latest = StableVersion.parse(REQUIRED_LATEST_RELEASE)
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
    if missing_commits:
        print(
            "Release target sanity check failed: release branch does not contain "
            "the required release commit(s): "
            f"{', '.join(missing_commits)}.",
            file=sys.stderr,
        )
        return 1
    if not release_tree_matches(args.head_ref):
        print(
            "Release target sanity check failed: release branch does not preserve "
            "the required release content manifest. Run "
            f"`just VERSION={required_target.tag()} release-target-manifest-update` "
            "and review the resulting checked-in digest when the release content "
            "was intentionally changed.",
            file=sys.stderr,
        )
        return 1
    print(f"Release target sanity check passed: {target.tag()}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
