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
    "57b5c7a7e178a5c386b9a477bf9c9f26a503fb23",  # POSIXレビュー正常終了時の子孫回収
    "daf4062c1ec285b70761ad695845e97816dcfbd8",  # macOS証跡のGraphviz準備と標準Bash
    "3d8974f19f5cf5d5e30bffa70c8c19e6594f9c8d",  # macOS証跡の子コマンドPATH隔離
    "5207512367ca49a3fc1c36ceb4afd9fcb10d6372",  # Windows弱フォント世代の実選択回帰
    "c0634a9fd02b87235be701ea167809ef1176facc",  # Windows Job回収のnative必須CI接続
    "39e8edd9561714c18b3ed12b81c18079e7393d9a",  # macOS証跡の認証dispatcherと共通PATH拘束
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
    "9c0a96d0e83063c148693cce0a531e1a19b99a1a",  # レビュー候補包含とorigin復号境界の拘束
    "7be38b373659fff340aaa7a5a9bb0eec73134368",  # SCP originのホスト名正規化
    "12583c6cc8f74553bb76aedfcc6e21b2dd6697be",  # フォントファイル世代によるglyph cache失効
    "169edd0d61322f838034efcfdb41a02a2dbffdbe",  # 調査したフォント候補世代によるHTML選択失効
    "780b0ebd2c7cc1d8450089d754e8f3a214448899",  # font-types0.12.6の最新互換依存更新
    "de3e0148158e7899b8fcfbfc6a95f373e08785f3",  # フォント復旧競合の決定的な検証
    "d61ae327d911866cfb1b3fb0667db67708e9da3d",  # cc1.6.0の最新互換依存更新
    "a10be620949ea3a6578d00013900ac968a953784",  # File世代確認とcmap読込の重複削減
    "4681663111bbbd9ab04e48a18120c1f32725656b",  # HTML描画内fallback選択の再利用
    "641a280714a82b29aa3064b72b668a578b09421a",  # Skrifa互換cmapの上限付きFile I/O再利用
    "24af66400c3995586e6e863a45198045217b21dc",  # usvgとSkrifaの文字対応政策を動的に揃えて更新
    "dae1c52f64e8212a694c87309174a9fe5e5d7384",  # 依存組更新の全言語契約を同期
    "d27c602f112503877fe4817075c93d014c49ac1c",  # 選択usvgの互換性を鮮度検査へ拘束
    "fae7f1d66d1b004b183420ed617c89080bfe3c73",  # 依存更新契約テストの必須整形
    "0d56c9cb1672f7e8af3492aba119fb2f85b42569",  # フォント世代とcmap回帰のOS非依存化
    "f6bc17870eb7d12d8381e24880877968335dee14",  # 各Refs句の全Issueを独立レビューへ拘束
    "cbe9b5c181ecaf39c6f4b1723ce47d39ba2b178d",  # 最後の候補を除外した空commitの証跡再利用を拒否
    "6081f03364ff62350b283d9fa069550ea8220887",  # フォント選択のOS非依存な境界回帰
    "5d75fb63b1c9d2b9bc9cb7dd443b617e5a668170",  # 非Unixの弱いFile世代の描画間キャッシュを抑止
    "f47cf564cff8351c9bf53630008ce5425b49cd2a",  # File書換回帰の全OS型検査と実行
    "da8566dbc0c6d2041bd88a7c0d885249ae7d2186",  # 弱いFile世代の正常probeとfault期待の整合
    "f036327b31f369a2762582f5636a604b2c8ee257",  # cmap保存条件と無効世代の除去回帰
    "c22b7044b02fdcb04e4d391aa59e122e42823446",  # stamp欠落時のglyph除去と別face保持の回帰
    "300eebc376be064571f3496afc7a574dcfe752b4",  # Drawio32.0.1資産・配布・文書同期
    "8a02cea8a558f44f864bcb8d6479f0a9de093672",  # Bezier曲線の実描画範囲計算
    "ab3f88cbe82f0b0f8a41f2412fefd5a59a8debc1",  # Mermaid参照生成の依存読込と失敗伝播修正
    "dede5dd2cdf205a94c1ac8c2660ceb6878ef00b5",  # Mermaid参照生成の回帰を標準ゲートへ接続
    "5512d3780a9b84f30e4799106706c593c95af3c9",  # Drawio 32.0.2 runtime資産と配布定義を同期
    "0f884a94727b4a736a6ef1b55339954b840782eb",  # 同一描画資産の分割と配布・内部依存契約
    "92a23c7fb1839a0e20962387190697a7b62b15d7",  # 検証済みHEADのpush拘束と軽量契約先行
    "551537a5a646d501b0ea1ba566045abe4cf36a53",  # 旧版の実package集合による公開再試行
    "288307beb032749326ee990fe0ae0793046a8545",  # fallback属性比較のOS共通回帰
    "1f4d7267cd821c30eed2e7c775cf1b94e14bf678",  # 保持レビュー要求のbranchとinput同一性拘束
    "abaa2a9aa07bb453b3d49e2b76b09553805fb79f",  # RTK有無によらない実Gitレビュー回帰
    "6e3b951a66bade2419faab37f8a863a6425a2996",  # Mermaid後半失敗時の参照資産復元
    "9f8cd468826ee3fa389d0ee3c270d0fb316af2e4",  # GitHubリポジトリ名の大小文字正規化
    "2b3ecb2fd02fc81277b1496631c88ae9be6fa18a",  # レビュー証跡の実HEAD祖先と整合性拘束
    "5c0e769688dc7befc55d0d651846654f2366df7e",  # 推定拒否回帰の明示レビュー環境からの分離
    "63365e78dfeef34ce16ca06c418c27156a657e00",  # 選択したpush remoteとbaseのレビュー拘束
    "b12cabf6887c57f20aab412203d68a1f70849776",  # 初回Draftレビューと指摘修正の品質検査を分離
    "fdc36ab410e94cf21b00b252d7f72c6b68d1597b",  # 初回PRレビューとReady後CIの手順統一
    "e87440867d436fe56d9fb23f9fee55337dc100e3",  # Draftの完全CI省略とReadyイベントの必須実行
    "e08f04f9acff6ecbd7aa5f8b231311e62580a6f2",  # 明示レビューと標準品質laneの接続回帰
    "4899718dd7eb1b1edf581d9fbe7bf2f075a0cc91",  # macOS動作保証対象をApple Siliconへ限定
    "0d8d9378bad199fac12e4711f5adb8fb6d986c11",  # レビュー中断時の子グループ終了と回収
    "f1c30c644a9c6ed23b252b074533204e62520fb2",  # ZenUML Core4.3.1の最新互換更新
    "9c3cb7613175a2f83018f997636a741aa5912cbb",  # 任意macOS証跡と必須Apple Silicon検査の分岐
    "af3e38c9c64f66862f8be33e0ccbfad4de2b1741",  # 保持したIssue文脈のHEAD祖先拘束
    "891c6ca1be2e866d94d538a5d179b3b1fe1aed8a",  # 任意macOS証跡のCI固定toolchain照合
    "51d1c83ba94a9f237216d262548f6bda1600ad44",  # SIGHUPでもレビュー子孫を回収
    "10db459396b3d8f84ad9a2760ce271bb728a4403",  # 任意証跡の公式stable Rust照合
    "5ac3a6cb96509f66912ff857474ad714540ab039",  # 実ファイルシステム精度に基づくフォント世代保存
    "fe7260ef1a79f9f9cbe5dd9c5cb39e3c3bc73317",  # macOS証跡の実ソースとARM対象・OS版を拘束
    "e296ba8292ac87294435f809204a4750b549901a",  # GitHub remoteの正当な末尾スラッシュを解析
    "d52468dfd4fa1705f7ace80afa1cd1a0cbbf6023",  # 証跡をGitフィルター非依存の実バイトと実行環境へ拘束
    "5f157dce81c2b190eba4e44d9b9afaa2030948ce",  # 親runtime環境の注入を拒否しvalidated Java21 JVMを固定
    "06f49830e9a0e72e86de43a0338a25ef27c167aa",  # 任意macOS証跡のCargo設定linker注入を拒否
    "c23f2453266bef3eacbe8269eec89b2f0e2ec1de",  # Cargo設定の依存差替え・検査条件変更・buildscript上書きを拒否
    "2c6d6cf2b11e52973a39c70dfce32cd6d033f1e1",  # Cargo取得元・別lockfileによる証跡混入を拒否
    "e0a30f0aff86f2569fea31a2859f700b27b6b96b",  # マージ済みレビュー文脈の再利用を拒否
    "037e04821414fb5b5e24a1095b78d84b7efc3e5e",  # Linux専用回帰の数値表記を厳格lintへ適合
    "aef3b250a91e51d728f541fa2e11d8a8b7de5aeb",  # 任意macOS証跡のJava配布元照合
    "b0a308ca70c620aaa0f9a31e118c5163b7bb7762",  # SIGQUIT時のレビュー子孫回収
    "08920462fbc8f9387e455350f32f66acd377a9dd",  # 選択GitHub remoteの同値URL正規化
    "54f5c1dea1a49b9564cd67111a0b1b014f8a0ba1",  # 実dispatchのClippyとrustfmt証跡検証
    "df187bc17086cca663a2974e1442a5e21f048da5",  # push品質検査をcommit済みbytesへ拘束
    "d1da51f4790999965d6ef6ae16ff2b0991d7bab2",  # reviewとIssue範囲検証のGit置換を無効化
    "0fbdde685d3a52cb26042e9f18ded241e6d20c18",  # 任意macOS証跡のGit置換とCargo aliasを拒否
    "4f33e16edb1deabea8e9130df9d7b0d2c8401b3a",  # 実行bit非対応filesystemのpre-push照合
    "313c3ba3e3289fd04d943c55462809f1907cdf5f",  # macOS証跡のignored依存を再構築
    "3fc7c0581d5ae4debc3727816bea1eccc6c431b4",  # 任意macOS証跡の成果物・依存キャッシュを隔離
    "8b7491070182f1a9ff0670c02c3fd08917cc4c63",  # RTKなしCIのレビューfixture契約
    "e4672623630580c3314df5b1fc2ff1c451c15616",  # 非キャッシュ環境のfont選択回帰契約
    "1b444141e38cf6dc592ce6e26c232f733ba218e5",  # Git標準CRLF checkoutのpush照合
    "da50738f7079c980ea52b2dc75cfad04d4136737",  # Windowsレビューの子孫回収と排他維持
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
REQUIRED_RELEASE_MANIFEST_SHA256 = "bfddc670c3fb8e11030816f825eb604d948bcd44f30f12b82eec61dc1c8592c5"
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
