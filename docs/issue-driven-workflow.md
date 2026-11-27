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

## 検証の収束運用

局所的な失敗やレビュー指摘の修正後は、失敗時と同じ入力・設定・実行条件で、再現できる最小の対象を検証する。条件が異なる場合は差分を記録し、元の条件で解消を確認する。

再実行前に、対象タスク、実行段階、所要時間、実際に止まった処理をローカルの検査証跡へ記録する。タイムアウトは停止時点の観測として扱い、その処理の計測と再現で原因を絞る。

同じ責務または失敗分類で二度連続して局所修正・再現検証が収束しない場合は、次の完全品質ゲートの前に隣接task、共通入力、共有不変条件、直近のレビュー指摘を点検する。必要なら原因分析を独立した責務として分離し、棄却した仮説と影響範囲を記録する。単に完全検査を再起動してこの点検を代替してはならない。

完全品質ゲートは、ソース変更と自己レビューの指摘を片付け、関連回帰を確認したコミットで実行する。既に有効な成功結果は、入力と証跡を既存の再利用契約で照合する。push後のDraftレビューは最新HEADで実施し、Ready化後に必須CIを確認する。
