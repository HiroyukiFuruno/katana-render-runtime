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
- subagent 起動時は利用可能モデルを確認し、限定実装は Luna を primary とし、利用できなければ Spark に切り替える。複雑な設計・統合分析や根本原因分析は Terra を使う。モデルと reasoning は明示する。
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

## 1.5 最新 HEAD のレビュー確認

レビュー依頼・再依頼の直前にGitHub APIでcurrent HEADとPR本文を再取得する。GitHub標準のレビュー結果と全ページ取得したthreadだけを確認する。

## 2. 内容評価と担当分解

`P0`、`P1`、`P2` 等は調査順序の手がかりであり、対応要否そのものではない。各指摘を次の観点で評価する。

1. 問題が現状で再現し、PR の目的・DoD に含まれるか。
2. 未対応時の実害と、互換性・性能・保守性への影響。
3. 今回の PR、別 PR、または別リポジトリのどこが責務を持つか。
4. 対応・見送りの根拠と、必要な検証。

P0/P1 は必ず内容を精査し、正当なら必須修正とする。不当、前提違い、またはスコープ外なら技術的根拠を返信する。評価後、分離可能な修正をファイル/責務ごとに subagent へ並列委譲し、main が差分を統合レビューする。

## 3. 修正と検証

- subagent には対象ファイル、変更可否、DoD、検証条件、禁止事項を短く明示する。
- 修正後、各担当の focused check に加え、main が差分・依存関係・回帰を確認し、repo の完全な品質ゲートを実行する。
- 不要な差分は早期に戻し、テスト都合で商用コードや品質基準を変更しない。

## 4. push、返信、スレッド解決


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


## 5. 最新 HEAD のレビュー反復

pushまたはPR本文変更後は、最新 HEAD に対して `@codex review` と自己レビューを再実施する。Issueコメント、formal reviews、review threadsを全ページ取得し、新しい指摘を個別に修正・reply・resolveする。



## 6. 完了判定

未解決thread 0、P0/P1対応済み、Issue/DoD、native required CI checksを確認してReady化する。merge直前にexpected HEAD SHAとrequired checksを再取得し、protected PR mergeを実行する。

## 継続実行と停止条件
