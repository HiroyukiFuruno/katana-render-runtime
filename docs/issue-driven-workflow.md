# Issue起点の変更契約

## 適用範囲

`master` 以外のbranchにある各commitは、このrepositoryのOPEN Issueをcommit messageから参照する。
短縮形は `Refs #64`、完全形は `Refs https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64` とする。

`pre-push` は次の順序を固定する。

1. repository固有の完全検査 `just check`
2. `scripts/hooks/verify_push_issue.py` によるIssue契約検査

Issueが存在しない、CLOSED、別repository、またはbranch固有commitの一部にIssue参照がない場合はpushを拒否する。

## 依存更新証跡

依存manifestまたはlockfileを変更する場合は、参照Issueへ両方の対象pathと次の節を記載する。
推移依存だけを更新してlockfileだけが変わる場合は、依存解決の起点となるmanifestをIssueへ記載すればよい。
API移行が不要な場合も、省略せず理由を記載する。

```markdown
## 依存更新証跡

- 上流公開版: `package-name 1.2.3` と公開URL
- API移行: 必要な変更、または移行不要の理由
- 依存manifest: `Cargo.toml` など変更した全path
- lockfile: `Cargo.lock` など変更した全path
- 検証証跡: 実行したcommandと成功結果
```

検証器は以下を拒否する。

- 上流公開版、API移行、manifest、lockfile、検証証跡の欠落
- 変更したmanifest / lockfile pathがIssue本文にない状態
- `TODO` / `TBD` のままの証跡

## Release cleanup

公開後cleanupは [リリース手順](release.md) の安全条件に従う。
作業中のworktreeを意図的に保持する場合は `git worktree lock <path>` でlocked状態にし、自動削除を拒否させる。

## Pull Requestレビュー運用

Pull Requestは、Issue契約とレビュー結果を確認してからReadyへ進める。レビューはGitHub標準の機能で運用する。

### 固定フロー

1. Pull RequestをDraftで作成する。
2. Draftの最新HEADに対して `@codex review` を依頼し、自己レビューも実施する。
3. GitHub APIで Issueコメント、formal reviews、review threads を全ページ取得し、指摘を個別に分類する。P0/P1は必ず修正し、P2/P3は要件・互換性・DoDへの影響を根拠付きで判断する。
4. 各指摘は個別に修正、検証、push、該当threadへのreply、threadのresolveまで完了する。HEADが変わった場合は最新HEADでレビューをやり直す。
5. 未解決threadが0で、Issue契約、DoD、native required CI checksが確認できたらReady化する。
6. Ready化後、merge直前にcurrent PRのexpected HEAD SHAとrequired checksを再取得し、protected PR mergeを実行する。人間による直接master更新やadmin bypassは使わない。

レビューやCIの待機中も、現在のHEADに対するローカル検証、Issue契約、依存関係調査、release/cleanup準備など、衝突しない作業を継続する。外部待機は完了条件の代替ではなく、個別の修正・検証・native checks・mergeをそれぞれ確認する。

レビュー完了の根拠は、最新HEADに紐づく全レビュー結果、全ページ取得したreview threads、個別reply/resolve、native required CI checks、Issue/DoDの検証結果とする。画面表示や単一のbotコメントだけを完了証跡にしない。
