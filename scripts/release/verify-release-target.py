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
PUBLISHED_V0423_SNAPSHOT = "8654789f69d24401b8ae67036db1cdb3e7f4294e"
REQUIRED_LATEST_RELEASE = "v0.4.23"
REQUIRED_TARGET_RELEASE = "v0.4.24"
REQUIRED_RELEASE_COMMITS = (
    "45dd933d81f5ccad2682b68b061eac39540ebcdd",
    "dd3269eeb203722985fdc6f2a03ee2ad666d0717",
    "f1e0daaf41bf578d2a05ae9a21cadd533f273174",
    "12bfd2eebd696cd3759a25c62f6b30b05f59b1db",
    "d3bf7a347e65893408ed4bdd8dd909a4d7f05658",
    "7656ebfa9f3108d295e8e3d8ebe41e70beaf2f0b",
    "9afddc49b92453d62413d48e016d16171c659b61",
    "1310d407ab1067cd5066a1485f9d90d4ee7e1f27",
    "d996b23c4a662e8d76fd4cebf415ee21c3630340",
    "d3d4c1a5ed8866a09756187e165ada94d2246a7c",
    "4a31dfa3f129efeffa7e8af3121885f14b82ac81",
    "f05dbae5726db1acd6e247f97e29e549e7c8f33a",
    "60da6dc876026d9268ec8efc8ca51257af6bf666",
    "e0c48147ad8c850dd078e5145696db263c6a0284",
    "4fb0d393cdb1f8f48a491a84d2c8b04b207bd97b",
    "79a8ba955b19e1f8b73e26b0aff5bbbe498f4240",
    "b3b939b8bc84fa4993d17e26bac505e9e4290683",
    "bcda50c4e9daa91014e45e78c74fdf76a9204416",
    "2cc08c7ec1c3c8f33c7080ba4802df8d2b69cac0",
    "3b62300c4ee4f14218a749654e479b9d08a7653f",
    "0eb33decbdb405d7c755a5ac6d2614361db8b980",
    "536658729364519837b50b10e1fb2ad8139ced1e",
    "dae31e9e94e2171d38b2950be302df1c032c7418",
    "930442f4e693462a058bf834d644e56c48577a98",
    "fcf9abd00e58c0da19584d2d602e23656984f944",
    "2dddb52c28cbf9cb9c424e885919a1d8e8ef5070",
    "a274d0421ffb13cac8c45d8a8a80de53a1bfd96c",
    "329f6b6c334e83a8784245bb240dfe5eadda64a2",
    "2220e789cd2efb0edae16827c921d321de1abbf1",
    "ff69082e2d670a795ec18c516a1ba0d214ad7abc",
    "5b1f38392546648b033776ca81b405036fe98f11",
    "838d1652f05d9799f4e0d9b0651a73ef9a880fd8",
    "8dfc8a98ac6094d26d55669f060d373029e20455",
    "52f40f603e7e9b94f522d73057d7d7dc4a97cc4c",
    "a8980ee0be22962581fc6b2b8651d56172e68002",
    "9a7ca67bf55a3e39ca5f4ca2a85fe7c442c0ee67",
    "8e0d8da9ed808b5c9685829b3a830faa5e48867d",
    "a786af868ebe19000ceacd76e33e42957ce59971",
    "44a50de256a887a20864a8bf76adf4768cc4d39e",
    "8ff5bf0aac12adb2c3101992308142e637489aa6",
    "4a28d34d873c225a9438cc7b6d5fb1f78cdf7f82",
    "863cfa03eac350da77ed3c4c393701002a0f2f81",
    "d6fb3c838599b4e48f945cc22eeaa5376046a9a7",
    "095f8bcac4297347eae47a34f72ff100526e1af6",
    "e804e70f2584c91f7c79937e43ca8e7d804f7642",
    "91f784ecc99b9e9e249488a64a6255083a32babf",
    "d0b0cc23c12d59647d882ad8040681243ac81bec",
    "ff27708e8f9b4adfbc7470b74dea742c4f9362fc",
    "b57760b6c7de1258d78eba0bad14471a4ef2824c",
    "f98d9cc9b4c06070ddacf600c9859cfa8ba94a0a",
    "6eeb2c1e541ed03b95b4274138755d75be767bea",
    "5ef3b20341f5bbc1e50e5e9ada8dec57375719bb",
    "8b13f5e195360c7169db821feb807a9379af1c71",
    "132c069b01f06efa56afbc9b22c7c11be39db08e",
    "cb45c106d71b6b7ed82d77ca87b5ae7bbc690952",
    "1de60f4631ca2673c492ef332c075d7d0bd0fb9e",
    "dedc5ad5653a4f31d4fc1fea94962f094e2823fe",
    "8fb679c284885145aba9e71eab5fb08eaf4a400d",
    "23dff6cdeaea7aa71ce903225bfae7c009a7dd64",
    "fdb6b761868e84162f39989f3635b152728465c6",
    "ebbbd899bafe0353d1196c8e13fbbda2eb4edef7",
    "8aef3d1f0e542bcb3ecd5683d4c671cfbf3b4f5d",
    "28a4cdc44d5cb7c16af41a5f9394fd5ae801466c",
    "8a787137858044588176e7faa8c0dff9854d7521",
    "09da6150a0b5c6c9c9f7b28b799e1b0338149911",
    "02c7963a75329d392575b4b69c760b96382197de",
    "3c02ed1651bd5adf536e414e53d702beb2556095",
    "e8ac617f639fcf7a44b9c7b538b420c75ed11190",
    "043aec401ac88de3f9795452205480a92bd8d6c0",
    "c5215a281321a00a8a9059c7e9fa36c10aa54c3d",
    "50e11e3c0ada72f12f26a110dc643bf5d4384b5a",
    "100dc7bef33ee24b858bd713c32d171df80cbe03",
    "ea1017f038805df371dafd106b2bbbd121372b49",
    "ac3de8e03842a3a7fbefcb432f5d29e76e9b92e1",
    "8b8b71f1c8a09ed1aac5da93a146921a99c2ae29",
)
# 公開snapshotが旧版の必須commitを全て含むため、今回のcommitだけを追加確認する。
REQUIRED_RELEASE_SOURCE_COMMITS = (
    PUBLISHED_V0423_SNAPSHOT,
    *REQUIRED_RELEASE_COMMITS,
)
REQUIRED_RELEASE_CANDIDATE_ANCESTORS = (
    PUBLISHED_V0423_SNAPSHOT,
    *REQUIRED_RELEASE_COMMITS,
)
# squash merge 後も default history に残る v0.4.23 release snapshotを、
# 変更集合の起点として使う。期待値そのものは commit/tree object ではなく、
# base からの non-gate 差分を表す不変な内容 manifest に固定する。
REQUIRED_RELEASE_BASE = PUBLISHED_V0423_SNAPSHOT
# Gate 修正はこの digest から除外するため、squash 後の gate repair によって
# 自己参照しない。候補source固定後にmanifest updateで新digestへ同期する。
# raw diff は path、file mode、base/target blob を含む。
REQUIRED_RELEASE_MANIFEST_SHA256 = "99a28f6d312c7bf6aa71b74f634a2339edcf2efb03a469dbe8cad0db786127e0"
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
    def parse(cls, value: str) -> StableVersion:
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
    if not required_commits or any(
        re.fullmatch(r"[0-9a-f]{40}", commit) is None for commit in required_commits
    ):
        return list(required_commits)
    head = subprocess.run(
        ["git", "rev-parse", "--verify", "--end-of-options", f"{head_ref}^{{commit}}"],
        check=False,
        capture_output=True,
        text=True,
    )
    resolved_head = head.stdout.strip()
    if (
        head.returncode != 0
        or not re.fullmatch(r"[0-9a-f]{40}", resolved_head)
        or (head.stdout and not head.stdout.endswith("\n"))
    ):
        return list(required_commits)
    result = subprocess.run(
        ["git", "rev-list", "--end-of-options", *required_commits, f"^{resolved_head}"],
        check=False,
        capture_output=True,
        text=True,
    )
    lines = result.stdout.splitlines()
    if (
        result.returncode != 0
        or (result.stdout and not result.stdout.endswith("\n"))
        or any(re.fullmatch(r"[0-9a-f]{40}", line) is None for line in lines)
    ):
        return list(required_commits)
    unreachable = set(lines)
    return [commit for commit in required_commits if commit in unreachable]


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
    for offset, raw_record in enumerate(records[:-1:2]):
        path = records[2 * offset + 1]
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
