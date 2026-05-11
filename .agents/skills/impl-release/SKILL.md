---
name: impl-release
description: katana-canvas-forge で指定バージョンの実装、品質確認、release branch PR 作成、cloud review 依頼、自動リリース確認までを一気通貫で進めるときに使う。/impl-release vX.Y.Z と同等のリリース実装ワークフロー。
---

# impl-release

`/impl-release vX.Y.Z` として扱う、katana-canvas-forge のリリース実装入口です。
この repository は `release/vX.Y.Z` から `master` へ取り込み依頼（Pull Request）を作り、merge 後に自動リリースします。

## 実行ルール

1. ユーザー指定の version を対象にする。例: `v0.1.0`
2. 作業開始前に `git status --short --branch` と `git fetch origin --prune --tags` を実行する。
3. 既存差分がある場合、release 作業へ混ぜる前に関心事を分ける。
4. 作業ブランチは `release/vX.Y.Z` に統一する。
5. 直接 `cargo publish` や tag 作成で迂回しない。公開は merge 後の自動実行基盤（GitHub Actions）に任せる。
6. 秘匿値（secret）は `CARGO_REGISTRY_TOKEN` を使う。値の取得や登録はユーザーが行う。

## Phase 1: 準備

```bash
git switch master
git pull --ff-only origin master
git switch -c release/vX.Y.Z
```

対象 version の OpenSpec change や tasks がある場合は、先に読みます。
見つからない場合は、release 内容を差分と `docs/release.md` から確認します。

## Phase 2: 実装と検証

未完了 task を実装し、必要に応じて `tasks.md` を更新します。
実装後は次を通します。

```bash
just check
just VERSION=vX.Y.Z release-check
git diff --check
```

失敗した場合は、除外や allow で逃げず、設計またはテストを直して同じ gate に戻ります。

## Phase 3: commit と push

`lefthook` を通すため、通常の commit / push を使います。

```bash
git status --short --branch
git add <release に必要な files>
git commit -m "release: vX.Y.Z リリース準備"
git push -u origin release/vX.Y.Z
```

`git push --no-verify` は使いません。

## Phase 4: PR 作成と cloud review

`release/vX.Y.Z` から `master` へ取り込み依頼（Pull Request）を作成します。
PR 作成前に、対象 version 以前の完了済み OpenSpec change を archive へ移動し、`lefthook run pre-pr` を通します。

```bash
lefthook run pre-pr
pr_url="$(gh pr create --base master --head release/vX.Y.Z --title "Prepare vX.Y.Z release" --body-file <pr-body-file>)"
gh pr comment "${pr_url}" --body '@codex review'
```

PR 作成後は必ず `@codex review` をコメントします。
レビュー（review）はローカルの自己レビューではなく cloud review を正とし、指摘は GitHub 上の review comment から取得して対応します。

## Phase 5: PR gate

cloud review は最低2回実施します。

1. 初回 review: PR 作成直後に `@codex review` を投稿する。
2. 最終 review: 初回指摘への対応後、または初回で指摘が無かった場合でも、merge 前にもう一度 `@codex review` を投稿する。

2回目以降で指摘が出た場合は対応し、指摘対応後にさらに1回 `@codex review` を投稿します。
完了条件は「最低2回実施」かつ「最後の cloud review で未対応の指摘がないこと」です。

次を確認します。

- `lint`
- `ast-lint`
- `test`
- `coverage`
- `release-preflight`
- 最後の cloud review の指摘が未対応でないこと

```bash
gh pr checks --watch "${pr_url}"
```

cloud review の指摘がある場合は修正し、通常の commit / push で更新します。
再レビューは任意ではなく、最後の修正 push 後に必ず同じ PR へ `@codex review` をコメントします。

## Phase 6: merge と自動リリース

すべての gate が通ったらユーザーに merge 承認を求めます。
承認後だけ merge します。

```bash
gh pr merge --merge --delete-branch "${pr_url}"
```

merge 後、Release workflow と crates.io 公開結果を確認します。

```bash
gh run list --workflow Release --limit 5
```

## 完了条件

- [ ] `release/vX.Y.Z` の PR が作成されている
- [ ] PR に `@codex review` コメントが最低2回投稿されている
- [ ] `lint`、`ast-lint`、`test`、`coverage`、`release-preflight` が通っている
- [ ] 最後の cloud review の指摘が解消されている
- [ ] merge 後に Release workflow が起動している
