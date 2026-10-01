---
name: create-pull-request
description: katana-diagram-renderer の Pull Request を、自己レビューと品質ゲート後に GitHub CLI で作る。base branch を文脈から確認し、PR 本文に検証結果を含める。
---

# Create Pull Request

PR 作成前に、差分、検証、base branch を確認します。PR は必ず Draft で作成し、cloud review と指摘対応を完了してから Ready にします。
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

PR本文の closing Issue 集合は commit 参照 Issue 集合と一致させ、Draftとして作成する。`isDraft=true`を確認したら、最新 HEAD のレビューへ進む。

## 5. Review と指摘対応

Draftの最新 HEAD に対して `@codex review` と自己レビューを実施し、Issueコメント、formal reviews、review threadsをGitHub APIで全ページ取得する。P0/P1は必ず修正し、P2/P3は根拠付きで判断する。各指摘は修正・検証・push・reply・resolveまで個別に完了し、HEAD変更後は最新HEADでレビューをやり直す。

## 6. Ready 化と承認後 merge

未解決thread 0、Issue契約、DoD、native required CI checksを確認してReady化する。Ready化後、merge直前にexpected HEAD SHAとrequired checksを再取得し、protected PR mergeを実行する。人間による直接master更新やadmin bypassは使わない。

## 報告

- PR URL
- base/head
- 検証結果
- CI 状態

## 継続実行と停止条件

レビュー指摘、CI、registry、cloud review の待機中も競合しない検証・証跡・cleanup準備を進める。結果取得後は Draft → 全件取得/分類 → 修正/検証/push → reply/resolve → 最新HEADレビュー → Ready → protected PR merge を完了する。
