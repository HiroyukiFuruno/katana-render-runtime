# Tasks: katana-diagram-renderer v0.1.1 reference score improvement

## Branch Rule

- **標準ブランチ**: `release/v0.1.1`
- **作業ブランチ**: `feature/v0.1.1-task-x`

---

## Release 判定

v0.1.1 は、Draw.io / Mermaid full compare 対象の 95 点以下を 0 件にした状態で release する。
99 点化は「公式を完全踏襲」と言える後続 change へ送る。

- [x] 95 点以下の残件を再評価し、0 件であることを確認した
- [x] 95〜99 点未満の残件を正式 report として `docs/releases/v0.1.1-reference-score-evaluation.md` に記録した
- [x] release 文言を「公式をなるべく忠実に踏襲」に限定する判断を記録した
- [x] `wireframe_1` の切り抜き崩壊を修正し、94.33795 から 99.891848 へ改善した
- [x] 99 点未満の 112 件は後続の完全踏襲 task として残す

---

## 1. Score Baseline Inventory

### 目的

v0.1.1 の release 判断に必要な score 未達を全 supported pattern から一覧化し、95 点以下の致命的な未達と 95〜99 点未満の後続対象を分ける。

### タスク

- [x] Draw.io full compare の全対象 directory を再評価した
- [x] Mermaid full compare の `en` と `ja` を再評価した
- [x] 95 点以下の case が 0 件であることを確認した
- [x] 95〜99 点未満の 112 件を正式 report に集約した
- [x] Draw.io representative baseline と Mermaid accepted score floor が 95 点未満を合格条件に使っていないことを確認した

### Definition of Done

- [x] Draw.io full の全対象 directory が確認済みである
- [x] Mermaid full の `en` と `ja` が確認済みである
- [x] 修正対象、release 許容対象、後続 change 対象が artifact に残っている
- [x] supported fixture を 95 点以下のまま完了扱いにしていない

---

## 2. Draw.io Score Improvement

### 目的

Draw.io の supported pattern に残る release 停止級の score 未達を renderer / resource / postprocess の責務で改善する。

### タスク

- [x] 95 点以下に残った `wireframe_1` を最小 directory で再現した
- [x] `wireframe_1` の原因を HTML fallback text の crop 対象混入と分類した
- [x] wrapped HTML fallback text を content crop から外す最小修正を行った
- [x] 回帰 test を追加した
- [x] `wireframe_1` を 99.891848 へ改善した
- [x] Draw.io full 再評価で 95 点以下 0 件を確認した

### Definition of Done

- [x] full compare 対象の supported Draw.io fixture に 95 点以下が残っていない
- [x] baseline を下げていない
- [x] 95 点未満 baseline で合格扱いにしていない
- [x] fallback SVG / stub PNG を追加していない
- [x] 95〜99 点未満の差分を report に残している

---

## 3. Mermaid Score Regression Check

### 目的

Mermaid の supported pattern に release 停止級の score 未達が残っていないことを確認する。

### タスク

- [x] Mermaid full compare を再評価した
- [x] Mermaid ZenUML fixture も supported fixture として評価した
- [x] Mermaid full 再評価で 95 点以下 0 件を確認した
- [x] 95〜99 点未満の差分を report に残した

### Definition of Done

- [x] full compare 対象の supported Mermaid fixture に 95 点以下が残っていない
- [x] `scripts/mermaid/reference_score_floors.ts` に 95 点未満 floor が残っていない
- [x] unsupported fixture を暗黙 skip していない

---

## 4. Quality Gate

### 目的

score 改善を通常品質ゲートと release gate に接続する。

### タスク

- [x] `just runtime-bundle-check` を実行した
- [x] `just fmt-check` を実行した
- [x] `cargo test -p katana-diagram-renderer page_layout_crop_tests -- --nocapture` を実行した
- [x] `just kdr-build` を実行した
- [x] `just drawio-compare-prebuilt tests/fixtures/drawio/official/templates/layout 0 tmp/kdr-v0.1.1-reference-score-improvement/drawio-layout-wireframe-le95-fix` を実行した
- [x] `just check` を実行する
- [x] `just VERSION=v0.1.1 release-check` を実行する
- [x] `git diff --check` を実行する

### Definition of Done

- [x] score 改善と通常品質ゲートが両方通っている
- [x] release-check が通っている
- [x] whitespace error が残っていない

---

## 5. Final Verification

- [x] `docs/releases/v0.1.1-reference-score-evaluation.md` を正式な git 管理 report として作成した
- [x] `npx -y @fission-ai/openspec validate "v0-1-1-reference-score-improvement" --strict` を実行する
- [x] `v0-1-1-reference-score-improvement` を archive する
- [ ] release branch を `origin` へ push する
- [ ] PR 作成後に `@codex review` を依頼する
