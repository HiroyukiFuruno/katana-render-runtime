---
name: kdr-workflow-guide
description: katana-diagram-renderer の開発で、OpenSpec、品質ゲート、自己レビュー、コミット、PR 作成を迷わずつなぐための案内スキル。大きい変更、バグ修正、品質確認、次に使うスキルの判断で使う。
---

# KDR Workflow Guide

このスキルは、katana-diagram-renderer（KDR）の repo-local skill を組み合わせる入口です。
KDR は Mermaid、Draw.io、ZenUML などの図表描画ランタイムと `kdr` CLI を扱うため、runtime asset、checksum、crate 境界、CLI 公開面を弱めないことを重視します。

## 1. 仕様から始める変更

変更が大きい、責務境界が曖昧、または利用者向けの公開 API が変わる場合は、先に OpenSpec で固定します。

1. `/openspec-propose`
   - `proposal.md`、`design.md`、仕様差分（specs）、`tasks.md` を作る。
2. `/openspec-apply-change`
   - `tasks.md` の単位で実装し、完了した項目だけ `[x]` にする。
3. `/openspec-verify-change`
   - 実装が仕様、設計、タスクと一致しているか確認する。
4. `/openspec-archive-change`
   - 実装と検証が終わった変更だけ archive へ移す。リリース対象より前の完了済み change は、release gate 前に archive して正式な release commit に含める。

通常の変更では PR 統合後に archive する運用も可能ですが、リリース作業では
`/impl-release` の対象 version より前の完了済み change を `release-check` と
`pre-pr` の前に archive します。archive の変更を正式な release commit に含め、
merge 後まで先送りしません。未完了の change は完了条件を満たすまで archive しません。

## 2. 日常的な実装変更

小さい修正でも、検証なしに進めません。

1. 変更前に `git status --short` で既存差分を見る。
2. バグ修正なら先に再現テストを追加する。
3. 変更後に `/lint-and-ast-lint` で必要な品質ゲートを通す。
4. `/self-review` で差分を見直す。
5. ユーザーがcommit/pushまたはrelease完了までの継続実行を明示した場合は `/commit_and_push` を使い、通常の中間承認待ちで止めない。未承認の外部公開、不可逆対象の未確定、実権限・秘密情報の不足、または成果を変える仕様判断だけを停止条件とする。

### Branch Policy

- 公開配布（crates.io）、release tag、公開 CLI、公開 API、package metadata に影響しない変更は `master` 直接作業でよい。
- 公開配布や release に影響する変更は、作業前に branch 方針を確認する。
- ユーザーが push を明示した場合は、ローカル commit で止めず、通常の `git push` まで実行する。
- pre-push が失敗した場合は回避せず、失敗した検査を修正してから再度 push する。

## 3. 一括変更

複数ファイルの置換、削除、移動、生成をまとめて行う場合は、先に `/bulk-modification-protocol` を使います。

- 事前に安全な差分か確認する。
- 大きな置換は責務ごとの小さい単位に分ける。
- 変更後は `git diff` を読み、消してはいけない理由や制約を巻き込んでいないか確認する。
- ファイル編集とコミットは同じ流れで続けない。検証と差分精査を完了してから、既承認の継続作業ならそのまま次工程へ進める。追加承認は不可逆対象の未確定または成果を変える判断に限る。

## 4. 品質ゲート

KDR の品質ゲートは、描画ランタイム、runtime asset、CLI、crate 公開面の安定性を守るために使います。

- `just fmt-check`
- `just lint`
- `just ast-lint`
- `just unit-test`
- `just runtime-bundle-check`
- `just biome`
- `just typecheck`
- `just runtime-asset-check`

`Justfile` に入口がある場合は、自己流コマンドではなく `just` の入口を優先します。

## 5. PR 作成

PR を作る前に `/self-review` と必要な品質ゲートを終えます。PR はDraftで作成し、最新 HEAD に対して `@codex review` と自己レビューを実施します。GitHub APIで Issueコメント、formal reviews、review threadsを全ページ取得し、指摘を個別に分類します。各指摘を修正・検証・pushし、該当threadへreplyしてresolveします。P0/P1は必須対応、P2/P3は根拠付きで判断します。未解決thread 0、Issue契約、DoD、native required CI checksを確認してReady化し、merge直前にexpected HEAD SHAとrequired checksを再取得してprotected PR mergeを実行します。

## 6. 持ち込まないもの

KDR には次の katana 固有スキルを持ち込みません。

- 画面 UI の手順
- 多言語翻訳
- アイコン管理
- changelog 作成
- アプリ固有のスクリーンショット運用

## 継続実行と停止条件

レビュー指摘、CI、registry、cloud reviewの待機中も競合しない調査・検証・Issue/PR証跡・cleanup準備を進め、結果取得後はDraft→全件取得/分類→修正/検証/push→reply/resolve→最新HEADレビュー→Ready→protected PR mergeを継続する。
