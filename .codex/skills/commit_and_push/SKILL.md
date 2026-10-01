---
name: commit-and-push
description: katana-diagram-renderer の変更を、検証、関心分離、自己レビューを済ませてから commit と push する。ユーザーが明示した場合だけ使う。
---

# Commit and Push

このスキルは、ユーザーがcommit/pushまたはrelease完了までの継続実行を明示したときに使います。
ファイル編集とコミットは同じ流れで連続させず、検証と差分精査を終えてから進める。継続実行が承認済みなら通常の中間承認待ちで止めず、不可逆対象の未確定、実権限・秘密情報の不足、または成果を変える仕様判断だけを停止条件とする。

## 1. 最初に確認する

```bash
git status --short
git diff --stat
```

- 他者の差分を混ぜない。
- 未追跡ファイルを黙って含めない。
- `.serena/`、`target/`、一時ファイルを含めない。
- ユーザーが指定した範囲だけを扱う。

commit 前に、対象変更に対応する同一 repository の canonical な OPEN Issue を選び、Issue 番号が正整数であることを確認します。以後の各 commit メッセージには、その Issue への `Refs #${issue_number}` を必ず含めます。Issue の選択・番号確認ができない場合は commit しません。

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
feat: renderer の公開 API を追加 Refs #${issue_number}
fix: Mermaid bundle の checksum 検証を修正 Refs #${issue_number}
docs: OpenSpec タスクを更新 Refs #${issue_number}
```

悪い例:

```text
fix: 色々修正 Refs #${issue_number}
feat: API と CLI とテストと文書をまとめて追加 Refs #${issue_number}
```

## 4. commit する

コミットメッセージは日本語にします。

```bash
git commit -m "<type>: <日本語の要約> Refs #${issue_number}"
```

`git commit --no-verify` は、コード変更を含む場合は使いません。
ドキュメントや OpenSpec のみで使う場合も、理由を報告します。

## 5. push する

```bash
git push
```

`git push --no-verify` は使いません。
hook 自体の不具合など例外が必要な場合は、理由、直前に通した検証、対象 commit を tasks.md または PR 本文に記録してからユーザーに確認します。

PR に紐付く push の直前には GitHub API から current PR の HEAD と本文を再取得し、最新状態を確認してから Draft のレビュー手順へ戻る。

## 6. PR 紐付き変更の後続フロー

push後もPRはDraftのまま維持する。最新 HEAD に対して `@codex review` と自己レビューを実施し、GitHub APIで Issueコメント、formal reviews、review threadsを全ページ取得する。各指摘を修正・検証・pushし、該当threadへreplyしてresolveする。P0/P1は必須対応、P2/P3は根拠付きで判断する。未解決thread 0、Issue/DoD、native required CI checks確認後にReady化し、merge直前にexpected HEAD SHAとrequired checksを再取得してprotected PR mergeを実行する。

## 報告

- commit hash
- push 先 branch
- 実行した検証
- 含めなかった既存差分
- PR の Draft/Ready 状態
- review 回数と対象となった最新 HEAD
- review thread の未 resolve 数
- CI の状態（green でも review 完了・Ready 条件とは別に報告する）
