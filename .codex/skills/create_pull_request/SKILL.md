---
name: create-pull-request
description: katana-diagram-renderer の Pull Request を Draft で作成し、Codex review と指摘対応、品質ゲートを完了してから Ready 化する。base branch を文脈から確認し、PR 本文に検証結果を含める。
---

# Create Pull Request

PR 作成前に、差分、検証、base branch を確認します。PR は必ず Draft で作成し、review と指摘対応を完了してから Ready にします。
推測で `master` や `main` を選びません。


## 1. 前提確認

```bash
git status --short
git branch --show-current
git branch -a
```

- commit 済みである。
- `/self-review` が完了している。
- `/lint-and-ast-lint` で必要な検証が通っている。
- 初回AIレビューはDraft作成後の `@codex review` に任せる。作成前は自己レビュー・必要な静的検査・関連回帰を確認し、通常pre-pushの品質laneを維持する。
- 未追跡や他者差分を混ぜていない。

## 2. base branch を決める

1. ユーザーが明示した base があればそれを使う。
2. OpenSpec の task branch なら、対応する integration branch を base にする。
3. integration branch 自体なら、通常は repository default branch を base にする。
4. 判断できない場合は、候補と理由を示してユーザーに確認する。

base branch の存在を確認します。

```bash
git branch -a | rg "<base-branch>"
```

## 3. PR template を確認する

```bash
test -f .github/PULL_REQUEST_TEMPLATE.md
```

template があれば優先します。
なければ次の形で本文を作ります。

```markdown
<!-- 日本語でレビューしてください。 -->

## 概要

## 対応内容

## 影響範囲

## 動作確認
```

## 4. Draft PR を作る

Draft PR を作る前に、branch の全 commit が参照する同一 repository の Issue 集合を収集します。PR 本文の closing Issue 集合は commit 参照 Issue 集合と完全一致させ、不足も余分も許可しません。各 Issue には GitHub closing keyword（`Closes #N`、`Fixes #N`、`Resolves #N`、または同一 repository の完全な Issue URL）による closing reference を含めます。`Refs #N` だけでは不十分です。

```bash
gh pr create --draft --base "<base-branch>" --head "<current-branch>" --title "<title>" --body-file "<body-file>"
```

`--base` は必須です。

作成直後に Draft 状態を機械確認します。`isDraft=true` でなければ、以降へ進みません。

```bash
gh pr view "<pr-number>" --json isDraft --jq '.isDraft'
```

## 5. Draft review と指摘対応

1. Draftではremoteの完全CIを起動しない。通常の `just check` と pre-push は品質laneを維持し、ローカルHigh reviewを自動で繰り返さない。必要時だけ `just local-review` を使う。
2. Draft の最新 HEAD に対して `@codex review` を依頼し、自己レビューも実施する。初回と再レビュー2回を通常の目安とする。
3. GitHub APIで Issue コメント、formal reviews、review threads を全ページ取得し、P0/P1/P2/P3に分類する。
4. 各指摘を個別に修正・検証し、push後に該当threadへreplyしてresolveする。P0/P1は必須対応、P2/P3は要件・互換性・DoDへの影響を根拠付きで判断する。不要な改善は理由を記録して後続patchへ送れる。修正時は自己レビュー、関連する静的検査と回帰確認を行い、静的解析だけで実動作を保証したとは扱わない。
5. HEADが変わった場合は最新 HEAD に対するPR reviewを取得する。レビュー回数だけを理由に最新HEAD review、未解決P0/P1、DoD違反、各threadのreply/resolve、required native checks、expected SHA付きmerge確認を省略しない。



## 6. Ready 化と承認依頼

最新HEADのreview取得・各threadへのreply/resolve・未解決thread 0・Issue契約・DoDを確認してから `gh pr ready` を実行する。`ready_for_review` で起動したnative required CI checksの成功を確認し、merge直前にcurrent PRのexpected HEAD SHAとrequired checksを再取得してprotected PR mergeを実行する。DraftでSKIPされたjobは品質成功の証拠にしない。人間による直接master更新やadmin bypassは使わない.

## 7. Ready 後確認

Ready PRのrequired CI、Issue契約、DoD、base/headを再確認する。merge結果はexpected HEAD SHAと保護されたPR mergeの結果で確認する。

## 報告

- PR URL
- base/head
- 最新 HEAD に対する review 結果
- 未resolve thread 数（0 件）
- self-review、lint、テスト、coverage、OpenSpec/DoD の検証結果
- CI 状態

## 継続実行と停止条件

レビュー指摘、CI、registry、cloud review の待機中も、競合しない検証・Issue証跡・cleanup準備を継続する。結果取得後は Draft → 全件取得/分類 → 修正/検証/push → reply/resolve → 最新HEADレビュー → Ready → protected PR merge を完了する。
