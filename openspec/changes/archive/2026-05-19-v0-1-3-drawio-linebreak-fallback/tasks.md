# Tasks: katana-diagram-renderer v0.1.3 Draw.io linebreak fallback

## 1. Scope And Issue

- [x] 1.1 GitHub issue #7 を確認する
- [x] 1.2 `v0.1.3` に Draw.io.js 30.0.2 更新済み差分を含める方針を確認する
- [x] 1.3 既存 active change `esm-imports-runtime-bundles` と本 change を混ぜない

## 2. OpenSpec

- [x] 2.1 `proposal.md` を作成する
- [x] 2.2 `design.md` を作成する
- [x] 2.3 `drawio-text-fallback-rendering` spec を作成する
- [x] 2.4 `runtime-asset-versioning` delta spec を作成する
- [x] 2.5 OpenSpec strict validate を通す

## 3. Implementation

- [x] 3.1 `&#10;` 改行が fallback `<text>` で1行に潰れる回帰 test を追加する
- [x] 3.2 修正前に回帰 test が失敗することを確認する
- [x] 3.3 fallback `<text>` に `<tspan>` が無い場合も行ごとの `<tspan>` を生成する
- [x] 3.4 `just runtime-bundle-build` で generated runtime bundle を再生成する
- [x] 3.5 workspace crate version を `0.1.3` に上げる

## 4. Verification

- [x] 4.1 Focused regression test を通す
- [x] 4.2 `just runtime-bundle-check` を通す
- [x] 4.3 `just check` を通す
- [x] 4.4 `just VERSION=v0.1.3 release-check` を通す
- [x] 4.5 `git diff --check` を通す

## 5. Release Flow

- [x] 5.1 完了した OpenSpec change を archive する
- [ ] 5.2 `lefthook run pre-pr` を通す
- [ ] 5.3 release branch を push する
- [ ] 5.4 `master` 向け PR を作成する
- [ ] 5.5 PR に `@codex review` を最低2回依頼する
- [ ] 5.6 PR gate 通過後、merge はユーザー承認後に行う
