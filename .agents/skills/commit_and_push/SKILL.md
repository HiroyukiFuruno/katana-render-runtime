---
name: commit_and_push
description: katana-diagram-renderer の変更を、検証、関心分離、自己レビューを済ませてから commit と push する。ユーザーが明示した場合だけ使う。
---

# Commit and Push

このスキルは、ユーザー明示の commit / push、またはそれを含む承認済み release 作業に使用します。
ファイル編集と commit は別 batch とし、検証結果を報告後、既存承認が対象範囲を満たす場合は再確認せず通常の commit / push へ進みます。未承認の公開・範囲拡大および App 操作固有の fresh 承認は維持します。

## 1. 最初に確認する

```bash
git status --short
git diff --stat
```

- 他者の差分を混ぜない。
- 未追跡ファイルを黙って含めない。
- `.serena/`、`target/`、一時ファイルを含めない。
- ユーザーが指定した範囲だけを扱う。

## 2. 検証する

変更内容に応じて `/lint-and-ast-lint` と `/self-review` を実行します。

標準:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`just lint`、`just ast-lint`、`make lint` が存在する場合は、自己流コマンドではなくそれを優先します。

検証が失敗した場合は commit しません。

## 3. 関心ごとに stage する

```bash
git add <file1> <file2>
git diff --cached --stat
git diff --cached
```

1 commit は 1 つの関心にします。

良い例:

```text
feat: renderer の公開 API を追加
fix: Mermaid bundle の checksum 検証を修正
docs: OpenSpec タスクを更新
```

悪い例:

```text
fix: 色々修正
feat: API と CLI とテストと文書をまとめて追加
```

## 4. commit する

コミットメッセージは日本語にします。

```bash
git commit -m "<type>: <日本語の要約>"
```

`git commit --no-verify` は、コード変更を含む場合は使いません。
ドキュメントや OpenSpec のみで使う場合も、理由を報告します。

## 5. push する

```bash
git push
```

`git push --no-verify` は使いません。
hook 自体の不具合など例外が必要な場合は、理由、直前に通した検証、対象 commit を tasks.md または PR 本文に記録してからユーザーに確認します。

## 報告

- commit hash
- push 先 branch
- 実行した検証
- 含めなかった既存差分
