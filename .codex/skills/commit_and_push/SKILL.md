---
name: commit-and-push
description: katana-diagram-renderer の変更を、検証、関心分離、自己レビューを済ませてから commit と push する。ユーザーが明示した場合だけ使う。
---

# Commit and Push

このスキルは、ユーザーが明示したときだけ使います。
ファイル編集とコミットは同じ流れで連続させず、検証結果を報告して承認を待ちます。

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

PR に紐付く push の直前には GitHub API から current PR の `head.sha` と `body` を再取得する。`body` は JSON string、NUL を含まない UTF-8 strict の文字列だけを受け入れ、`hashlib.sha256(body.encode("utf-8", "strict")).hexdigest()` の64桁小文字hexを記録する。同一 HEAD で本文 digest が既存 marker と異なる場合、旧reviewとtrusted successは失効しているため、push前でも Ready 化・merge を進めない。

## 6. PR 紐付き変更の後続フロー

PR に紐付く変更では、push 成功や CI green だけで review 完了・Ready 条件成立とは扱いません。push 後も PR は Draft のまま維持し、次の review 循環へ戻ります。

最初の initial review は `create_pull_request` スキルが起点にするが、このスキルによる後続 push も新しい initial review を起点にする。

1. push 後、GitHub API から current PR の `head.sha` と `body` を再取得し、NUL・non-string・UTF-8 strict 不能を fail-closed で拒否する。marker は属性順序と空白を含めて `^<!-- krr-review phase=(?:initial|final) head=[0-9a-f]{40} body-sha256=[0-9a-f]{64} -->$` に完全一致させる。本文 digest は `hashlib.sha256(body.encode("utf-8", "strict")).hexdigest()` のみを使う。

2. 新しい push は新しい HEAD と current 本文に対する **initial review** を必ず開始する。以前の final reviewを同一修正循環へ持ち越さない。

   ```text
   <!-- krr-review phase=initial head=<40-lowerhex> body-sha256=<64-lowerhex> -->
   @codex review
   ```

3. initial review の指摘を解消して thread reply/resolve と検証を完了したら、marker投稿直前に current PR の HEAD・本文を再取得して同じdigestを再計算し、次の final review を依頼する。取得値が initial marker と異なる場合は final を投稿せず、新しい initial review からやり直す。

   ```text
   <!-- krr-review phase=final head=<40-lowerhex> body-sha256=<64-lowerhex> -->
   @codex review
   ```

4. initial/final のいずれでも新規指摘が出たら、分離可能な修正を subagent に委譲し、修正 → push → 新HEAD・本文の initial review から反復する。同一 HEAD でも body edit により digest が変われば、旧 marker、review、trusted success を完了扱いにしない。
5. 修正した各 review thread に、対応内容と検証結果を reply し、確認できた thread だけを resolve する。
6. review thread の未 resolve 数が 0 であることを確認する。CI green だけで review 完了・Ready 条件成立とは扱わない。
7. 最新 HEAD・本文 digest の review 完了、未 resolve 0、必要な CI/品質ゲート確認を満たしたら、main が機械ゲートを実行する。success を再利用する writer/trusted Check Run evidence の query にある current `pr_body_sha256` は、GitHub APIから再取得した current PR body を正規化せず strict UTF-8でSHA-256化したdigestと**ちょうど1個（exactly one）**一致しなければならない。missing、duplicate、stale、または異なるdigestはfail-closedで拒否し、旧successを再利用せずReady化・mergeへ進まない。成功したらmainは `gh pr ready <number>` でPRをReady化する。Ready化の後に初めてユーザーへfreshなmerge承認を求める。承認後はmerge直前に同じ `just pr-ready-check "<number>"` を再実行し、global bootstrap skillの `merge --apply` だけでmergeする。UI・ブラウザ・通常のGitHub CLI merge（`gh pr merge`）・人間/admin bypassは禁止する。このスキル自体はReady化・mergeコマンドを実行しない。

P1 などの review 指摘修正を実装する場合、分離可能ならファイルまたは責務単位で subagent に委譲し、main はオーケストレーターとして要件・DoD・差分・検証を統合確認する。同じファイルや責務を重ねて委譲しない。

Ready 判断前の機械ゲートは `just pr-ready-check "<number>"` とする。この local gate は参照Issueが OPEN であること、依存更新証跡が揃っていること、PR range の Issue contract が完全一致すること（不足・余分を含む）を先に検証する。

## no-issues の Issue comment 証跡

`Reviewed commit` の prefix は current HEAD に一致させます。

review botの「no issues」はformal reviewではなく、trusted botがPR Issueへ投稿したcanonical commentだけを完了証跡として受理します。本文は `Codex Review: Didn't find any major issues...` または同じcanonical prefixの短文に続き、`**Reviewed commit:** \`<10〜40桁の小文字SHA prefix>\`` を含み、current HEADがそのprefixで始まらなければなりません。`created_at == updated_at` を必須とし、phase marker間のcanonical候補は高々1件、duplicateはfail-closedで拒否します。任意のdetailsは省略またはcanonicalな1つだけ（summary `ℹ️ About Codex in GitHub`、本文8192文字以内）を許可し、nested details、details外のclosing/sentinel文字列、重複canonical行は拒否します。reactionや任意のbot commentは証跡にしません。finalのno-issues証跡は参照Issueの最終`updated_at`より後でなければなりません。Codexが同一HEADの同一結果をduplicate suppressionした場合だけ、initial/final markerが同じHEAD・本文digestで、unresolved threadが0、かつIssue更新後かつfinal marker前に記録されたcanonical commentをinitial-to-final evidenceとして再利用できます。通常のformal reviewまたは指摘対応経路では、final marker後に別のreview完了証跡を取得します。

## 報告

- commit hash
- push 先 branch
- 実行した検証
- 含めなかった既存差分
- PR の Draft/Ready 状態
- review 回数と対象となった最新 HEAD
- review thread の未 resolve 数
- CI の状態（green でも review 完了・Ready 条件とは別に報告する）
