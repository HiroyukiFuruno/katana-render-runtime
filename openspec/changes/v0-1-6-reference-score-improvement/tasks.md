# Tasks: katana-canvas-forge v0.1.6 reference score improvement

## Branch Rule

- **標準ブランチ**: `release/v0.1.6`
- **作業ブランチ**: `feature/v0.1.6-task-x`

---

## 1. Score Baseline Inventory

### 目的

v0.1.0 release 時点で残る score 未達を全 supported pattern から一覧化し、Jules CLI が修正対象を迷わない状態にする。

### 書き込み範囲

- `tmp/kcf-v0.1.6-reference-score-improvement/`
- `tests/fixtures/drawio/representative/score-baseline.json`
- `openspec/changes/v0-1-6-reference-score-improvement/tasks.md`

### タスク

- [ ] 1.1 `mkdir -p tmp/kcf-v0.1.6-reference-score-improvement` を実行する
- [ ] 1.2 `just drawio-compare-ci 99` を実行し、失敗した fixture 名、score、出力先を `tmp/kcf-v0.1.6-reference-score-improvement/inventory.md` に記録する
- [ ] 1.3 `just drawio-compare-full 99` を実行し、全対象 directory ごとに失敗した fixture 名、score、出力先を `inventory.md` に追記する
- [ ] 1.4 `just mermaid-compare-ci 99` を実行し、supported fixture の状態を `inventory.md` に追記する
- [ ] 1.5 `just mermaid-compare-full 99` を実行し、`en` と `ja` の supported fixture の状態を `inventory.md` に追記する
- [ ] 1.6 Mermaid ZenUML は v0.1.2 対応済みの supported fixture として、score と出力先を `inventory.md` に記録する
- [ ] 1.7 `tests/fixtures/drawio/representative/score-baseline.json` に 99 未満の score がある場合、全件を `inventory.md` に記録する
- [ ] 1.8 `scripts/mermaid/reference_score_floors.ts` に 99 未満の floor がある場合、全件を `inventory.md` に記録する

### Definition of Done

- [ ] `inventory.md` に `score < 99` の修正対象が case 単位で並んでいる
- [ ] Draw.io full の全対象 directory が確認済みである
- [ ] Mermaid full の `en` と `ja` が確認済みである
- [ ] 修正対象、保留対象、後続 change 対象が artifact に残っている
- [ ] supported fixture を 99 未満のまま完了扱いにしていない

---

## 2. Draw.io Score Improvement

### 目的

Draw.io の全 supported pattern の score 未達を renderer / resource / postprocess の責務で改善する。

### 書き込み範囲

- `crates/katana-canvas-forge/src/markdown/drawio_renderer/`
- `scripts/drawio/`
- `tests/fixtures/drawio/`
- `tmp/kcf-v0.1.6-reference-score-improvement/`

### タスク

- [ ] 2.1 `i18n` を含む最小 directory として `tests/fixtures/drawio/official/diagrams` を選び、`just drawio-compare tests/fixtures/drawio/official/diagrams 99 tmp/kcf-v0.1.6-reference-score-improvement/i18n` を実行する
- [ ] 2.2 `tmp/kcf-v0.1.6-reference-score-improvement/i18n/comparison` の official PNG、kcf PNG、diff、report の `i18n` 行だけを見て差分を一種類だけ分類する
- [ ] 2.3 `i18n` の分類が `resource` なら resource resolver、`postprocess` なら SVG postprocess、`runtime` なら runtime adapter だけを最小修正する
- [ ] 2.4 `i18n` を再実行し、score が 99 以上になるまで 2.2 から繰り返す
- [ ] 2.5 `link` を含む最小 directory として `tests/fixtures/drawio/official/diagrams` を選び、`just drawio-compare tests/fixtures/drawio/official/diagrams 99 tmp/kcf-v0.1.6-reference-score-improvement/link` を実行する
- [ ] 2.6 `link` も 2.2 から 2.4 と同じ cycle で 99 以上へ上げる
- [ ] 2.7 representative baseline の 99 点未満 case を `inventory.md` から 1 case ずつ選び、`just drawio-compare tests/fixtures/drawio/representative 99 tmp/kcf-v0.1.6-reference-score-improvement/<case>` で再現する
- [ ] 2.8 各 case は「再現、差分分類、最小修正、再比較、score 99 以上確認」の順で 1 case ずつ閉じる
- [ ] 2.9 full compare だけで出た 99 未満 case も、対象 directory ごとに 1 case ずつ 2.2 から 2.4 と同じ cycle で閉じる
- [ ] 2.10 改善後、`tests/fixtures/drawio/representative/score-baseline.json` の下限を 99 以上へ上げる
- [ ] 2.11 `scripts/drawio/reference-score.ts` が 99 未満 baseline を合格条件に使えない設計になっていることを確認し、必要なら修正する
- [ ] 2.12 `just drawio-compare-ci 99` が通ることを確認する
- [ ] 2.13 `just drawio-compare-full 99` が通ることを確認する

### Definition of Done

- [ ] full compare 対象の supported Draw.io fixture に score 99 未満が残っていない
- [ ] baseline を下げていない
- [ ] baseline を 99 未満にして合格扱いにしていない
- [ ] fallback SVG / stub PNG を追加していない
- [ ] contact sheet で改善前後を確認できる
- [ ] 原因が分からない case は broad refactor せず、見えている差分、触ったファイル、次に疑う場所を report に残して停止している

---

## 3. Mermaid Score Regression Check

### 目的

Mermaid の全 supported pattern も v0.1.6 の 99 点基準にそろえる。

### 書き込み範囲

- `scripts/mermaid/`
- `tests/fixtures/mermaid/`
- `tmp/kcf-v0.1.6-reference-score-improvement/`

### タスク

- [ ] 3.1 `just mermaid-compare-ci 99` を実行する
- [ ] 3.2 `just mermaid-compare-full 99` を実行する
- [ ] 3.3 supported fixture の score 99 未満が出た場合は、Draw.io と同じ「case 切り出し、差分分類、最小修正、再比較」cycle で直す
- [ ] 3.4 ZenUML fixture も supported fixture として score 99 以上を確認する
- [ ] 3.5 `scripts/mermaid/reference_score_floors.ts` の 99 未満 floor は supported fixture の合格条件に残さない
- [ ] 3.6 Mermaid score policy の baseline が不必要に緩んでいないことを確認する

### Definition of Done

- [ ] full compare 対象の supported Mermaid fixture は score 99 以上で通っている
- [ ] `scripts/mermaid/reference_score_floors.ts` に supported fixture の 99 未満 floor が残っていない
- [ ] unsupported fixture が暗黙 skip されていない

---

## 4. Quality Gate

### 目的

score 改善を通常品質ゲートと release gate に接続する。

### タスク

- [ ] 4.1 `just drawio-compare-ci 99` を実行する
- [ ] 4.2 `just drawio-compare-full 99` を実行する
- [ ] 4.3 `just mermaid-compare-ci 99` を実行する
- [ ] 4.4 `just mermaid-compare-full 99` を実行する
- [ ] 4.5 `just check` を実行する
- [ ] 4.6 `just VERSION=v0.1.6 release-check` を実行する
- [ ] 4.7 `git diff --check` を実行する

### Definition of Done

- [ ] score 改善と通常品質ゲートが両方通っている
- [ ] compare command はすべて min score 99 で実行している
- [ ] CI compare だけでなく full compare を完了判定にしている

---

## 5. Final Verification

- [ ] 5.1 `/lint-and-ast-lint` 相当の検査結果を記録する
- [ ] 5.2 `/self-review` 相当の検査で「full compare 対象の supported fixture に score 99 未満が残っていない」ことを確認する
- [ ] 5.3 PR 作成後に `@codex review` を依頼する
- [ ] 5.4 `npx -y @fission-ai/openspec validate "v0-1-6-reference-score-improvement" --strict` を実行する
- [ ] 5.5 指摘があれば対応し、再度 `@codex review` を依頼する
