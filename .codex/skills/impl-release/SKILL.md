---
name: impl-release
description: katana-diagram-renderer で指定バージョンの実装、品質確認、release branch PR 作成、自動リリース確認までを一気通貫で進めるときに使う。/impl-release vX.Y.Z と同等のリリース実装ワークフロー。
---

# impl-release

`/impl-release vX.Y.Z` として扱う、katana-diagram-renderer のリリース実装入口です。
この repository は `release/vX.Y.Z` から `master` へ取り込み依頼（Pull Request）を作り、merge 後に自動リリースします。
初回公開版は `v0.1.0` から開始します。


## 実行ルール

1. ユーザー指定の version を対象にする。例: `v0.1.0`
2. 作業開始前に `git status --short --branch` と `git fetch origin --prune --tags` を実行する。
3. 既存差分がある場合、release 作業へ混ぜる前に関心事を分ける。
4. 作業ブランチは `release/vX.Y.Z` に統一する。
5. 直接 `cargo publish` や tag 作成で迂回しない。公開は merge 後の自動実行基盤（GitHub Actions）に任せる。
6. 秘匿値（secret）は `CARGO_REGISTRY_TOKEN` を使う。値の取得や登録はユーザーが行う。
7. 不自然な version 飛び番は停止し、`just VERSION=vX.Y.Z release-target-check` の結果を確認する。
8. ユーザーが release 完了までの継続実行を承認している場合、通常のcommit、push、Draft review、Ready化、gate、公開確認、Issue更新、cleanupごとに再承認を求めて停止しない。停止は不可逆対象の未確定、実際の権限・秘密情報不足、またはversion/公開内容を変える判断だけに限定し、待機中は独立した検証と次版準備を並列で進める。

## Phase 1: 準備

```bash
git switch master
git pull --ff-only origin master
just VERSION=vX.Y.Z release-target-check
git switch -c release/vX.Y.Z
```

対象 version の OpenSpec change や tasks がある場合は、先に読みます。
見つからない場合は、release 内容を差分と `docs/release.md` から確認します。

対象 version より前の完了済み OpenSpec change は、`release-check` と `pre-pr` の前に archive へ移動します。archive の変更も release の正式な commit に含め、merge 後まで先送りしません。未完了の change は完了条件を満たすまで archive せず、対象 version の release gate を通してはいけません。

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

`lefthook` を通すため、通常の commit / push を使います。commit 前に、対象変更に対応する同一 repository の canonical な OPEN Issue を選び、Issue 番号が正整数であることを確認します。以後の各 commit メッセージには、その Issue への `Refs #${issue_number}` を必ず含めます。Issue の選択・番号確認ができない場合は commit しません。

```bash
git status --short --branch
git add <release に必要な files>
git commit -m "release: vX.Y.Z リリース準備 Refs #${issue_number}"
git push -u origin release/vX.Y.Z
```

`git push --no-verify` は使いません。

## Phase 4: Draft PR 作成と cloud review

Draft PRを作成し、最新 HEAD に対して `@codex review` と自己レビューを実施する。GitHub APIで Issueコメント、formal reviews、review threadsを全ページ取得し、指摘を個別に分類する。

## Phase 5: PR gate

各指摘を個別に修正・検証・pushし、該当threadへreplyしてresolveする。P0/P1は必須対応、P2/P3は要件・互換性・DoDへの影響を根拠付きで判断する。HEADが変わった場合は最新 HEADでレビューをやり直す。

## Phase 6: Ready 化と merge 承認

最新HEADレビュー、未解決thread 0、Issue契約、DoDを確認してReady化する。Readyで起動したrequired CI checksの成功を確認し、DraftのSKIPを品質成功として扱わない。Ready化後、merge直前にcurrent PRのexpected HEAD SHAとrequired checksを再取得し、protected PR mergeを実行する。人間による直接master更新やadmin bypassは使わない。

## Phase 7: merge と自動リリース




merge 後、Release workflow と crates.io 公開結果を確認します。

```bash
gh run list --workflow Release --limit 5
```

## 完了条件

- [ ] `release/vX.Y.Z` の PR が作成されている
- [ ] Draft PR が確認され、初回 `@codex review` が投稿されている
- [ ] 指摘ごとに subagent で修正し、thread reply / resolve が完了している
- [ ] 最新 HEAD に対する review 結果を確認している
- [ ] 未 resolve thread が 0 件である
- [ ] `Test and Build (...)` と `preflight` が通っている
- [ ] OpenSpec の tasks / DoD と `release-target-check` が通っている
- [ ] Ready 化前に全 review thread を確認し、未 resolve thread が 0 件である
- [ ] release-check / pre-pr の前に対象 version 以前の完了済み OpenSpec change を archive している
- [ ] merge 後に Release workflow が起動している

- [ ] Rust/JS の依存を最新互換版まで確認・更新し、完全品質ゲートを通している
- [ ] Release workflow、GitHub Release、crates.io の対象 version 公開を確認している
- [ ] 対応 Issue を公開後の DoD 完了時に close し、`branch-hygiene` に従って不要 branch/worktree/stash を整理している

## 継続実行と停止条件

レビュー指摘、CI、registry、cloud review の待機中も競合しない調査・検証・Issue証跡・cleanup準備を進める。結果取得後は Draft → 全件取得/分類 → 修正/検証/push → reply/resolve → 最新HEADレビュー → Ready → protected PR merge → release確認を完了する。
