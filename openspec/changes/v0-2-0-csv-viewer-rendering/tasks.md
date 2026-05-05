# Tasks: katana-canvas-forge v0.2.0 CSV viewer rendering

## Branch Rule

- **標準ブランチ**: `release/v0.2.0`
- **作業ブランチ**: `feature/v0.2.0-task-x`

---

## 1. Scope Baseline

### 目的

v0.2.0 を CSV viewer rendering に限定し、export や spreadsheet engine へ広げない。

### タスク

- [ ] 1.1 v0.1.0 から v0.1.4 までの transfer / score / runtime asset / ZenUML / viewer e2e が前提であることを確認する
- [ ] 1.2 viewer rendering と export の責務差を確認する
- [ ] 1.3 CSV 編集、保存、型変換を non-goal として固定する
- [ ] 1.4 v0.5.0 まで public CLI contract を固定しない方針を確認する

### Definition of Done

- [ ] CSV viewer rendering の目的と非目的が artifact に残っている
- [ ] KatanA UI state を kcf に持ち込まない制約が明確である

---

## 2. Viewer Rendering Boundary

### 目的

CSV viewer rendering を library API として呼べる中立境界にする。

### タスク

- [ ] 2.1 `ViewerRenderInput` / `ViewerRenderOptions` / `ViewerRenderOutput` を設計する
- [ ] 2.2 CSV renderer が KatanA UI 型を参照しないことを確認する
- [ ] 2.3 artifact kind、HTML fragment、metadata、diagnostics を返す
- [ ] 2.4 CLI は検証用 entrypoint に留め、公開 contract にしない

### Definition of Done

- [ ] KatanA 側 adapter が DTO だけで表示へ接続できる
- [ ] core library に viewer UI 依存がない

---

## 3. CSV Parser And Rendering

### 目的

CSV を構造として parse し、見やすい table artifact に変換する。

### タスク

- [ ] 3.1 delimiter hint と delimiter 推定を実装する
- [ ] 3.2 quoted field、comma inside field、field 内改行を処理する
- [ ] 3.3 header 有無を option と metadata で扱う
- [ ] 3.4 表示用 HTML escape を実装する
- [ ] 3.5 値の自動型変換を行わないことを test で固定する

### Definition of Done

- [ ] CSV の主要 pattern が table artifact として render される
- [ ] parse error が暗黙 fallback されない

---

## 4. Windowing And Large File Handling

### 目的

巨大 CSV でも全量 DOM 化せず viewer に渡せるようにする。

### タスク

- [ ] 4.1 row / column offset と limit を実装する
- [ ] 4.2 既定上限を設定する
- [ ] 4.3 truncation metadata と diagnostics を返す
- [ ] 4.4 large fixture で memory / time smoke を追加する

### Definition of Done

- [ ] 大きな CSV でも既定範囲だけを render する
- [ ] 省略された範囲が metadata で分かる

---

## 5. Fixtures And Tests

### 目的

CSV viewer rendering の回帰を fixture で検知する。

### タスク

- [ ] 5.1 `tests/fixtures/csv/` を追加する
- [ ] 5.2 normal、quoted、multiline、wide、large、encoding fixture を追加する
- [ ] 5.3 expected metadata と HTML fragment を検証する
- [ ] 5.4 viewer e2e に CSV case を追加する

### Definition of Done

- [ ] unit / integration / viewer e2e の責務が分かれている
- [ ] fixture が CI で再現可能である

---

## 6. Final Verification

- [ ] 6.1 `just check` を実行する
- [ ] 6.2 `cargo tree --workspace -e normal` で KatanA UI 依存がないことを確認する
- [ ] 6.3 CSV viewer e2e smoke を実行する
- [ ] 6.4 `npx -y @fission-ai/openspec validate v0-2-0-csv-viewer-rendering --strict` を実行する
- [ ] 6.5 変更差分を自己レビューする
