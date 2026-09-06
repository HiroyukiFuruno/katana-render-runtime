---
name: gh-address-comments
description: PRのレビュー指摘を取得・分類し、内容評価に基づく修正、検証、返信、GraphQLでのスレッド解決、最終ゲートまでを行う。
metadata:
  short-description: Draft PRレビュー指摘の取得から解決・最終ゲートまでのKRRフロー
---

# PRレビュー指摘対応ワークフロー

KRR の PR レビュー指摘は、PR を Draft のまま維持し、全スレッドを内容評価したうえで対応する。CI green だけでは完了としない。

## 役割と前提

- GitHub CLI (`gh`) が認証済みで、対象リポジトリの作業ディレクトリにいることを確認する。
- main は司令塔として、全 thread の取得、P0/P1 を含む内容評価、担当分解、統合判断、最終ゲートを担う。
- 分離可能な修正実装は、ファイルまたは非重複責務単位で subagent へ最大並列委譲する。同じファイル・責務を重ねて割り当てない。
- subagent 起動時は利用可能モデルを確認し、限定実装は Spark が利用できなければ Luna、複雑な設計・統合分析は Terra を使う。モデルと reasoning は明示する。
- 正当でスコープ内の指摘は、ユーザー確認を待って停滞しない。確認はスコープ外、不可逆操作、または選択により結果が変わる場合だけ行う。
- 実装前に指摘、DoD、担当、検証条件を作業台帳へ固定し、未対応と対応済みを区別する。

## 1. 全レビュー指摘を取得

REST の review comments だけでなく、reviews と reviewThreads を確認し、ページングを尽くして取りこぼしを防ぐ。REST の review comments は `--paginate` を使う。reviews は REST 一覧で代用せず、GraphQL connection の `pageInfo` / `endCursor` を使って最後のページまで取得する。`scripts/fetch_comments.py` が利用可能なら、同スクリプトの仕様を確認して優先する。

```bash
gh api --paginate repos/{owner}/{repo}/pulls/{pr_number}/comments
gh api graphql -f query='
query($owner:String!, $repo:String!, $number:Int!, $reviewsCursor:String, $threadsCursor:String) {
  repository(owner:$owner, name:$repo) {
    pullRequest(number:$number) {
      reviews(first:100, after:$reviewsCursor) {
        pageInfo { hasNextPage endCursor }
        nodes { id author { login } state body submittedAt }
      }
      reviewThreads(first:100, after:$threadsCursor) {
        pageInfo { hasNextPage endCursor }
        nodes { id isResolved isOutdated path line comments(first:100) {
          pageInfo { hasNextPage endCursor }
          nodes { id databaseId body author { login } }
        } }
      }
    }
  }
}' -f owner='{owner}' -f repo='{repo}' -F number='{pr_number}'
```

`reviews.pageInfo.hasNextPage` と `reviewThreads.pageInfo.hasNextPage` はそれぞれ独立した cursor で反復し、`hasNextPage=false` まで全ページを取得する。`hasNextPage=true` なのに `endCursor` が空、または既出 cursor を返した場合は無限ループを避けてエラーにし、Ready 化を止める。

各 review thread の `comments.pageInfo` も必ず確認する。`hasNextPage=true` なら thread node を起点に comments connection の残ページを GraphQL で取得して統合する。残ページ取得を実装できない状況では、先頭100件だけを完全な結果と見なさず、fail-closed でエラーにして Ready 化を止める。

取得結果を thread 単位に重複排除し、未resolve・outdated・返信済みを明示する。

## 1.5 review marker の本文束縛

marker の投稿と cloud review 依頼の直前に、GitHub API から対象 PR を再取得する。`head.sha` と `body` をその時点の値で固定する。`body` は JSON string であること、NUL を含まないこと、UTF-8 strict で符号化できることを確認し、次の **本文バイト列そのもの** の SHA-256 小文字16進表現を使う。

```python
body_sha256 = hashlib.sha256(body.encode("utf-8", "strict")).hexdigest()
```

marker は次の完全一致の文法だけを使う。属性順序・空白・phase 名を変えない。`^<!-- krr-review phase=(?:initial|final) head=[0-9a-f]{40} body-sha256=[0-9a-f]{64} -->$` に一致しなければならない。

```text
<!-- krr-review phase=initial head=<40-lowerhex> body-sha256=<64-lowerhex> -->
<!-- krr-review phase=final head=<40-lowerhex> body-sha256=<64-lowerhex> -->
```

marker の投稿と review 依頼の間に HEAD または本文が変わった可能性があれば、再取得して marker を作り直す。本文は HEAD を変えずに編集できるため、同一 HEAD でも digest が異なる既存 marker・review・trusted success は無効である。本文が変わった場合は Draft を維持したまま新しい `initial` marker を投稿し、初回 review からやり直す。本文や marker の不正・取得不能は fail-closed とし、Ready 化しない。

## 2. 内容評価と担当分解

`P0`、`P1`、`P2` 等は調査順序の手がかりであり、対応要否そのものではない。各指摘を次の観点で評価する。

1. 問題が現状で再現し、PR の目的・DoD に含まれるか。
2. 未対応時の実害と、互換性・性能・保守性への影響。
3. 今回の PR、別 PR、または別リポジトリのどこが責務を持つか。
4. 対応・見送りの根拠と、必要な検証。

P0/P1 は必ず内容を精査し、正当なら必須修正とする。不当、前提違い、またはスコープ外なら技術的根拠を返信する。評価後、分離可能な修正をファイル/責務ごとに subagent へ並列委譲し、main が差分を統合レビューする。

## 3. 修正と検証

- PR は Draft のまま保持する。Ready 化や merge はこの skill の範囲外で、別の governance gate に委ねる。
- subagent には対象ファイル、変更可否、DoD、検証条件、禁止事項を短く明示する。
- 修正後、各担当の focused check に加え、main が差分・依存関係・回帰を確認し、repo の完全な品質ゲートを実行する。
- 不要な差分は早期に戻し、テスト都合で商用コードや品質基準を変更しない。

## 4. push、返信、スレッド解決

検証が通った修正を commit・push した後、各 thread に具体的な返信を行う。修正時は変更内容と検証、見送り時は技術的根拠、質問時は回答を簡潔に記す。**修正の push は旧 marker・review・trusted success を無効化するため、reply / resolve の完了後に 1.5 の current HEAD/body strict `initial` marker と cloud review を完了させてから final review へ進む。** 同一 HEAD の PR本文編集も同じく initial review から反復する。REST API は返信に使えるが、thread の resolve には GraphQL を使う。

返信後は `pull_request_review_id` の `state` と `submitted_at` を確認する。`PENDING` なら、自分が今回の修正に対して作成した返信だけであることを確認してから review event `COMMENT` として submit し、approval と混同しない。他人の返信や未知の draft を無断で公開しない。submit 後は独立した read で公開状態を確認する。App の read で返信が見えないだけでは権限を増やさず、既に `COMMENTED` なのに見えない場合も対象 comment、pagination、取得 scope を先に調査する。

```bash
gh api repos/{owner}/{repo}/pulls/{pr_number}/comments/{comment_id}/replies \
  -X POST -f body="対応しました。{変更内容と検証結果}"
```

```bash
gh api graphql -f query='
mutation($threadId:ID!) {
  resolveReviewThread(input:{threadId:$threadId}) {
    thread { id isResolved }
  }
}' -f threadId='{thread_node_id}'
```

返信対象と resolve 対象を thread ごとに記録し、各指摘が返信済み・resolve 済み（見送りも根拠返信済み）であることを確認する。

reply / resolve が完了しても trusted success を自動的に有効とは扱わない。最終反復へ進む前に、trusted Check Run evidence の query を重複を保持して読み、`pr_body_sha256` が **ちょうど1個** だけ存在し、current PR本文から1.5で再計算した digest と完全一致することを確認する。missing、duplicate、old digest、異なるdigestは fail-closed とし、修正の push または本文編集後は新しい initial marker の cloud review 完了前に final marker を投稿しない。

## 5. 最新 HEAD・本文の最終レビュー反復

**push 後に final cloud review へ直行しない。** まず 1.5 の current HEAD/body strict `initial` marker と cloud review を完了させる。同一 HEAD の PR本文編集でも同じ initial review の再開が必要である。initial review、全指摘の reply / resolve、検証の完了後に限り、最終 cloud review を依頼する直前にPRの current HEAD と本文を再取得し、latest initial marker と完全一致することを確認したうえで、1.5 の厳密な検証と UTF-8 SHA-256 を行う。次の形式で最終 cloud review を依頼する。

```text
<!-- krr-review phase=final head=<40-lowerhex> body-sha256=<64-lowerhex> -->
@codex review
```

旧 HEAD の review、または同一 HEAD でも旧本文 digest の review は無効として扱う。新規指摘があれば 2〜5 を繰り返し、修正、検証、push、返信、resolve、current HEAD・本文の strict initial review 完了、最新 HEAD・本文に束縛した最終 review の順に完了する。

## no-issues の Issue comment 証跡

`Reviewed commit` の prefix は current HEAD に一致させます。

review botの「no issues」はformal reviewではなく、trusted botがPR Issueへ投稿したcanonical commentだけを完了証跡として受理します。本文は `Codex Review: Didn't find any major issues...` または同じcanonical prefixの短文に続き、`**Reviewed commit:** \`<10〜40桁の小文字SHA prefix>\`` を含み、current HEADがそのprefixで始まらなければなりません。`created_at == updated_at` を必須とし、phase marker間のcanonical候補は高々1件、duplicateはfail-closedで拒否します。任意のdetailsは省略またはcanonicalな1つだけ（summary `ℹ️ About Codex in GitHub`、本文8192文字以内）を許可し、nested details、details外のclosing/sentinel文字列、重複canonical行は拒否します。reactionや任意のbot commentは証跡にしません。finalのno-issues証跡は参照Issueの最終`updated_at`より後でなければなりません。Codexが同一HEADの同一結果をduplicate suppressionした場合だけ、initial/final markerが同じHEAD・本文digestで、unresolved threadが0、かつIssue更新後かつfinal marker前に記録されたcanonical commentをinitial-to-final evidenceとして再利用できます。通常のformal reviewまたは指摘対応経路では、final marker後に別のreview完了証跡を取得します。

## 6. 完了判定

最後に `just pr-ready-check "{pr_number}"` を実行し、最新 HEAD・本文 digest の post-marker bot review、未resolve thread 0、CI、DoD を機械確認する。この local gate は参照Issueが OPEN であること、依存更新証跡が揃っていること、PR range の Issue contract が完全一致すること（不足・余分を含む）も先に検証する。writer/trusted Check Run の success evidence は、query中の current `pr_body_sha256` が **ちょうど1個** で完全一致する場合だけ有効である。欠落、duplicate、old digest、差異はいずれも fail-closed であり success と見なさない。CI green のみ、レビュー依頼済みのみ、または局所テスト通過のみでは完了としない。Ready化後はユーザーから**freshなmerge承認**を得て、同じ `just pr-ready-check "{pr_number}"` を直前に再実行してReady PRを再検証する。成功時のmergeはPR外のglobal skill `/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/SKILL.md` が定める専用Appの `merge --apply` だけを使い、人間、UI、通常のGitHub CLI merge、admin bypassは禁止する。`prepare --apply` は保存済みの人間用`gh auth`だけを使う例外であり、activate/merge/finalize/verifyのlive operationはfresh JWTとscript自身がmint・検証するApp IATを必要とする。`merge` の`--apply`なしdry-runだけは、人間用authによるpublic readに限定する。pr-ready-check 成功後も、Draft 維持のまま main が最終差分と結果を報告する。
