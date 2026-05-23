# リリース手順

## 方針

`katana-render-runtime` は実装の本体として公開し、`katana-diagram-renderer` への互換ルートは公開対象に含めません。

`release/vX.Y.Z` ブランチから `master` へ取り込み依頼（Pull Request）を作る。
その取り込み依頼（Pull Request）では通常の品質ゲート（quality gate）とリリース前検査を必須にする。
PR 作成前には `lefthook run pre-pr` を実行し、対象版番号（version）以前の完了済み OpenSpec change が `openspec/changes/archive/` に移動済みであることを確認する。
取り込み（merge）後は自動実行基盤（GitHub Actions）がタグ（tag）、GitHub リリース（GitHub Release）、crates.io 公開を実行する。

## 必須検査

GitHub のブランチ保護（branch protection）では、少なくとも次を必須検査（required check）にする。

- `Test and Build (macos-latest)`
- `Test and Build (ubuntu-latest)`
- `Test and Build (windows-latest)`
- `preflight`

## リリース前検査

`release-preflight` は `release/v...` ブランチの取り込み依頼（Pull Request）で `just release-check` を実行する。
`just release-check` は、release 予定版番号（version）以前の OpenSpec change が active 側に残っている場合に失敗する。
内容は次の通り。

- 版番号（version）が GitHub Release / remote tag 上の自然な次版であること。`v0.3.0` は rename release として KDR の版番号を引き継ぐ
- 対象タグ（tag）が remote 上の既存タグを上書きしないこと
- 対象版番号（version）が crates.io に未公開であること
- 整形確認（format）、静的検査（lint）、単体テスト（unit test）、抽象構文木検査（AST lint）
- KatanA UI 依存の混入検知（dependency leak）
- カバレッジ（coverage）。行カバレッジ（line coverage）100%、未到達行（uncovered line）0
- runtime bundle の同期、TypeScript 型検査、runtime asset の checksum 確認
- `Cargo.toml` の版番号（version）と branch 版番号（branch version）の一致
- 作業領域（workspace）内部依存の版番号（version）一致
- `katana-render-runtime` の梱包（package）と公開の事前実行（publish dry-run）
- `katana-render-runtime-cli` の梱包（package）収録対象確認

`katana-render-runtime-cli` は `katana-render-runtime` を先に公開しないと crates.io 上で依存解決できないため、
取り込み依頼（Pull Request）時点では `katana-render-runtime` を事前実行（dry-run）し、CLI は収録対象確認までに留める。
公開順序は `katana-render-runtime`、`katana-render-runtime-cli` の順に固定する。

## 公開順序

取り込み（merge）後の `Release` ワークフロー（workflow）は次の順で動く。

1. `just release-target-check`
2. `just release-verify`
3. リリースタグ（release tag）作成
4. GitHub リリース（GitHub Release）作成
5. `katana-render-runtime` を crates.io に公開
6. crates.io で `katana-render-runtime` が見えるまで待機
7. `katana-render-runtime-cli` を crates.io に公開

## 必要な秘匿値

自動実行基盤（GitHub Actions）には `CARGO_REGISTRY_TOKEN` が必要。
値は crates.io の API トークン（API token）を使う。

ユーザーが実行する登録コマンド:

```bash
cd /Users/hiroyuki_furuno/works/private/katana-render-runtime
gh secret set CARGO_REGISTRY_TOKEN
```

`Cargo` は crates.io のトークン（token）を `CARGO_REGISTRY_TOKEN` 環境変数で受け取れる。
トークン（token）は秘匿値として扱い、リポジトリ（repository）に保存しない。

参考:

- https://doc.rust-lang.org/cargo/reference/config.html#credentials
- https://doc.rust-lang.org/cargo/commands/cargo-publish.html
