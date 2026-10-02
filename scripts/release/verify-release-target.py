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

PUBLISHED_V0422_SNAPSHOT = "185d056de67282a4056729296e57c164a6a343d6"
PUBLISHED_V0422_RELEASE_MANIFEST_SHA256 = "1ddeddc37c0735a7278755f26f8985aee9176fc978b437766d57be7d0e342d9e"
REQUIRED_LATEST_RELEASE = "v0.4.22"
REQUIRED_TARGET_RELEASE = "v0.4.23"
REQUIRED_RELEASE_COMMITS = (
    "322a85eb75892944c009231b534885601553c488",  # Issue #89のCI品質契約
    "1410802e9d63c8e1c3347a24dad858de52523612",  # Issue #95のglyph cache修正
    "cf15441d102924a3b8e254b7ab9fc377a7daade3",  # v0.4.23のversion metadataと固定契約
    "2e7b5c7312abc72bd7db8fcd8b5fc3c36f5cc079",  # release対象契約の認証harness
    "7489943dcdc797ba73bbde0fd9b9e348a731127c",  # Issue #89のhook検証契約修正
    "5df6d489f0f6987e0e91a2bec9c9761c80bdd3e4",  # Issue #89のindex commit alias修正
    "e77a463e37ae8cea0f4e5d38b53694d1b8aa9b3e",  # Issue #89のhead/index差分検証
    "c87876eb0c3f8406b1487857e62a30b442f894a4",  # Issue #89のCI環境対応
    "85df066a5e38395b4a6088b28440c8ec0a4afba1",  # Issue #89の複数Issue証跡再利用
    "74a974874b3c4f54a2df0993cb6fc6e5ab8b1750",  # Issue #89のレビュー原本照合
    "4531fdb6bd6a18283886130a9a607305bff38d20",  # Issue #89の全証跡推定原本照合
    "2a14f27e80d91725d6a24db4b45136f8cf62382a",  # Issue #89のGit変換とobject形式照合
    "e6b7e0ffaed736b53081d388555ceedef945dc13",  # 最新Mermaid12.1.0の依存更新
    "45a4952f3ae45c2d05901b160ab638059b56c058",  # Mermaid12.1.0の公式参照・文書同期
    "0f89e24c6754e57273420670163d59f64f3afb91",  # 新規ファイルのpush包含をレビュー証跡へ固定
    "676fcb3ede5c1079495f5e90101e7821e072c82a",  # GitHub SSH URIの互換性修正
    "cdbe8d15af5ee67be2d46868ffc4a8c27b293a56",  # origin制御文字と実Git改行境界の拒否
    "91ac5111d69e911d2c4d8043d6cfae96f9947b4e",  # SSH originの空delimiterと空portの拒否
    "d4742624989c4afac1f0a85780fcc591ae5b218d",  # SSH URIのGit互換復号
    "c406e9eadb4638a413e7b04cc0a9e4f7baeefa6d",  # OS所有のレビュー排他と異常終了回復
    "d3f04d90618f10869d2a98a31bc6c7392b77fbf0",  # Rollup4.64.0の最新互換依存更新
    "451573aeb68ad95830ed78c05cc021961bf20a00",  # hidden index・owner execute・Git-zの正確な証跡拘束
    "7e8c0d2a894512ea0e6e4456ebb7e7abf1faa098",  # Cargo環境のpresence・compiler alias・target優先順の拘束
    "20b44b2d599be4802c66e2ddab401067e411495b",  # Cargoの空target-dirを証跡正規化前に拒否
    "63339c19d0963f739c2fb751dcd04d9879168751",  # 作業ツリーのGit clean変換後blobをレビュー証跡へ拘束
    "652508bd51a8f8dee49d43ccb888a14df2e75630",  # libc0.2.190の最新互換依存更新
    "27b9272c2d2bee9f14ab6336e3a6fa5d0c4727fa",  # パス種別変更とHTTPS既定ポートのレビュー互換性
    "73df5c6a934b40cc166d709ceb6b184ea057f10f",  # 一時フォント障害の選択を描画内で再利用
    "d72b80e802d9b857eb71503ad1fce6cdf3f5fcf0",  # 非UTF8のGitパスを可逆なレビュー証跡へ保持
    "c8704bf9b2cc26c43f85f0e9dad35f31ee11d4e0",  # 非UTF8のGit名回帰をPOSIX環境へ限定
)
# 公開snapshotと#89/#95および正式metadata commitをsource/candidate両方で要求する。
REQUIRED_RELEASE_SOURCE_COMMITS = (
    PUBLISHED_V0422_SNAPSHOT,
    *REQUIRED_RELEASE_COMMITS,
)
REQUIRED_RELEASE_CANDIDATE_ANCESTORS = (
    PUBLISHED_V0422_SNAPSHOT,
    *REQUIRED_RELEASE_COMMITS,
)
# squash merge 後も default history に残る v0.4.22 release snapshotを、
# 変更集合の起点として使う。期待値そのものは commit/tree object ではなく、
# base からの non-gate 差分を表す不変な内容 manifest に固定する。
REQUIRED_RELEASE_BASE = PUBLISHED_V0422_SNAPSHOT
# Gate 修正はこの digest から除外するため、squash 後の gate repair によって
# 自己参照しない。候補source固定後にmanifest updateで新digestへ同期する。
# raw diff は path、file mode、base/target blob を含む。
REQUIRED_RELEASE_MANIFEST_SHA256 = "6d98e9b86d7c2355ae9a7e184918623ff765f69081d9052b28cc08a76fb785a0"
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
    head_ref: str,
    required_commits: tuple[str, ...] = REQUIRED_RELEASE_CANDIDATE_ANCESTORS,
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
        missing_source_commits = missing_required_commits(
            args.head_ref, REQUIRED_RELEASE_SOURCE_COMMITS
        )
        if target != required_target or missing_source_commits:
            print(
                "Release target manifest update failed: target must be "
                f"{required_target.tag()} and head must contain the required release source commit(s).",
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
    missing_candidate_ancestors = missing_required_commits(args.head_ref)
    if missing_candidate_ancestors:
        print(
            "Release target sanity check failed: release branch does not contain "
            "the required release candidate ancestor(s): "
            f"{', '.join(missing_candidate_ancestors)}.",
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
