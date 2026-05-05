---
name: create_pull_request
description: katana-canvas-forge の Pull Request を、自己レビューと品質ゲート後に GitHub CLI で作る。base branch を文脈から確認し、PR 本文に検証結果を含める。
---

# Create Pull Request

PR 作成前に、差分、検証、base branch を確認します。
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

## 4. PR を作る

```bash
gh pr create --base "<base-branch>" --head "<current-branch>" --title "<title>" --body-file "<body-file>"
```

`--base` は必須です。

## 5. PR 後確認

```bash
gh pr view --web
gh pr checks
```

CI が失敗した場合は、`gh-fix-ci` 相当の調査に進みます。

## 報告

- PR URL
- base/head
- 検証結果
- CI 状態
