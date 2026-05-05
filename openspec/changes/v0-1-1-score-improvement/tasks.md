# Tasks: katana-canvas-forge v0.1.1 score improvement

## Branch Rule

- **標準ブランチ**: `release/v0.1.1`
- **作業ブランチ**: `feature/v0.1.1-task-x`

---

## 1. Score Baseline Inventory

### 目的

v0.1.0 release 時点で残る score 未達を一覧化し、Jules CLI が修正対象を迷わない状態にする。

### タスク

- [ ] 1.1 Draw.io representative の既知下限を一覧化する
- [ ] 1.2 Draw.io full compare の early-fail 箇所を一覧化する
- [ ] 1.3 Mermaid supported fixture の compare 状態を一覧化する
- [ ] 1.4 Mermaid ZenUML は v0.1.3 対象として除外理由を記録する

### Definition of Done

- [ ] 修正対象、保留対象、後続 change 対象が artifact に残っている

---

## 2. Draw.io Score Improvement

### 目的

Draw.io の score 未達を renderer / resource / postprocess の責務で改善する。

### タスク

- [ ] 2.1 `i18n` の差分原因を調査する
- [ ] 2.2 `link` の差分原因を調査する
- [ ] 2.3 representative baseline の 99 点未満 case を原因別に分類する
- [ ] 2.4 修正可能な差分を実装する
- [ ] 2.5 改善後の baseline を上げる

### Definition of Done

- [ ] baseline を下げていない
- [ ] fallback SVG / stub PNG を追加していない
- [ ] contact sheet で改善前後を確認できる

---

## 3. Mermaid Score Regression Check

### 目的

v0.1.1 の Draw.io 修正で Mermaid の score 運用を壊していないことを確認する。

### タスク

- [ ] 3.1 supported Mermaid fixture の compare を実行する
- [ ] 3.2 ZenUML fixture は v0.1.3 の未対応として report に残す
- [ ] 3.3 Mermaid score policy の baseline が不必要に緩んでいないことを確認する

### Definition of Done

- [ ] supported Mermaid fixture の score regression がない
- [ ] unsupported fixture が暗黙 skip されていない

---

## 4. Quality Gate

### 目的

score 改善を通常品質ゲートと release gate に接続する。

### タスク

- [ ] 4.1 `just drawio-compare-ci` を実行する
- [ ] 4.2 `just drawio-compare-full` を実行する
- [ ] 4.3 `just check` を実行する
- [ ] 4.4 `just VERSION=v0.1.1 release-check` を実行する
- [ ] 4.5 `git diff --check` を実行する

### Definition of Done

- [ ] score 改善と通常品質ゲートが両方通っている

---

## 5. Final Verification

- [ ] 5.1 `/lint-and-ast-lint` 相当の検査結果を記録する
- [ ] 5.2 PR 作成後に `@codex review` を依頼する
- [ ] 5.3 `npx -y @fission-ai/openspec validate "v0-1-1-score-improvement" --strict` を実行する
- [ ] 5.4 指摘があれば対応し、再度 `@codex review` を依頼する
