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
5. ユーザーが明示した場合だけ `/commit_and_push` を使う。

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
- ファイル編集とコミットは同じ流れで続けない。検証結果をユーザーに報告してから承認を待つ。

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

PR を作る前に `/self-review` と必要な品質ゲートを終えます。
PR 作成は `/create_pull_request` に委譲し、Ready PR を直接作成しません。次の状態遷移を厳守します。

1. Draft PR を作成し、`isDraft=true` を機械確認する。
2. Draft のまま初回 `@codex review`（cloud review）を、依頼直前にGitHub APIからcurrent PRのHEAD/bodyを再取得して生成した `krr-review phase=initial head=<40 lowercase hex> body-sha256=<64 lowercase hex>` marker とともに依頼し、review / thread / コメントを取得・分類する。bodyはstringであることを確認し、NULまたはlone surrogateを含む場合、またはUTF-8 strict encodeに失敗する場合はdigest計算前にfail-closedで停止する。正常なbodyは正規化せずUTF-8 bytesとしてSHA-256化する。
3. 修正が必要な指摘は、対象ファイルまたは責務ごとに重複なく subagent へ並列委譲する。
4. 各指摘を修正・検証し、該当 thread へ対応内容を reply してから resolve する。
5. 指摘の有無や修正 push の有無にかかわらず、依頼直前に再取得した最新HEAD/bodyに対する最終 cloud review を、`krr-review phase=final head=<40 lowercase hex> body-sha256=<64 lowercase hex>` marker とともに必ず依頼し、結果を取得する。最終 review で新規指摘が出た場合は、対象ごとに subagent で修正・検証し、push、該当 thread への reply / resolve を行った後、更新後のcurrent HEAD/bodyを再取得して再度reviewする。PR bodyを編集した場合は同じHEADでも旧markerと旧reviewを無効とし、initial marker→bot review→final marker→bot reviewをやり直す。このサイクルを未 resolve thread 0 かつ新規指摘なしになるまで反復する。
6. 最新HEADのbot review完了、未resolve thread 0、CI / DoD PASSを確認し、`just pr-ready-check "<pr>"` を実行する。local gateは参照IssueのOPEN、依存更新証跡、PR rangeのIssue contract完全一致（不足・余分なし）を先に検証し、markerのHEAD/body digestとtrusted evidenceのHEAD/`pr_body_sha256`/external_id一致も確認する。trusted Check Run queryの`pr_body_sha256`は、GitHub APIから再取得したcurrent PR bodyを正規化せずstrict UTF-8でSHA-256化したdigestと**ちょうど1個（exactly one）**一致しなければならない。missing、duplicate、stale、または異なるdigestはfail-closedで拒否する。
7. `pr-ready-check` 成功後だけ `gh pr ready` でReady化し、ユーザーから**freshなmerge承認**を得る。承認後に同じ `just pr-ready-check "<pr>"` を再実行し、Ready PRの最新Issue/marker/thread/CI/base/headとtrusted digest bindingを再検証する。成功時のmergeはPR外のglobal skill `/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/SKILL.md` が定める専用Appの `merge --apply` だけを使う。人間、UI、通常のGitHub CLI merge、admin bypassは禁止する。`prepare --apply` は保存済みの人間用`gh auth`だけを使う例外であり、activate/merge/finalize/verifyのlive operationはfresh JWTとscript自身がmint・検証するApp IATを必要とする。`merge` の`--apply`なしdry-runだけは、人間用authによるpublic readに限定する。承認前にmergeしない。OpenSpecのDoD確認からDraft PR、Ready化、release導線まで同じmarker・trusted `pr_body_sha256`一意一致契約を維持し、旧successを再利用しない。

CI green だけでは review 完了、Ready 化、または merge の条件を満たしません。self-review、lint、テスト、coverage、OpenSpec / DoD、最新 HEAD の cloud review、未 resolve thread 0 を個別に確認します。

## no-issues 証跡の共通契約

`Reviewed commit` の prefix は current HEAD に一致させます。

review botの「no issues」はformal reviewではなく、trusted botがPR Issueへ投稿したcanonical commentだけを完了証跡として受理します。本文は `Codex Review: Didn't find any major issues...` または同じcanonical prefixの短文に続き、`**Reviewed commit:** \`<10〜40桁の小文字SHA prefix>\`` を含み、current HEADがそのprefixで始まらなければなりません。`created_at == updated_at` を必須とし、phase marker間のcanonical候補は高々1件、duplicateはfail-closedで拒否します。任意のdetailsは省略またはcanonicalな1つだけ（summary `ℹ️ About Codex in GitHub`、本文8192文字以内）を許可し、nested details、details外のclosing/sentinel文字列、重複canonical行は拒否します。reactionや任意のbot commentは証跡にしません。finalのno-issues証跡は参照Issueの最終`updated_at`より後でなければなりません。Codexが同一HEADの同一結果をduplicate suppressionした場合だけ、initial/final markerが同じHEAD・本文digestで、unresolved threadが0、かつIssue更新後かつfinal marker前に記録されたcanonical commentをinitial-to-final evidenceとして再利用できます。通常のformal reviewまたは指摘対応経路では、final marker後に別のreview完了証跡を取得します。

## 6. 持ち込まないもの

KDR には次の katana 固有スキルを持ち込みません。

- 画面 UI の手順
- 多言語翻訳
- アイコン管理
- changelog 作成
- アプリ固有のスクリーンショット運用
