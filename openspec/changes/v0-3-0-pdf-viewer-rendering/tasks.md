# Tasks: katana-canvas-forge v0.3.0 PDF viewer rendering

## Branch Rule

- **標準ブランチ**: `release/v0.3.0`
- **作業ブランチ**: `feature/v0.3.0-task-x`

---

## 1. Scope Baseline

### 目的

v0.3.0 を PDF viewer rendering に限定し、PDF export や PDF 編集へ広げない。

### タスク

- [ ] 1.1 v0.1.x と v0.2.0 が前提であることを確認する
- [ ] 1.2 PDF export と PDF viewer rendering の責務差を確認する
- [ ] 1.3 PDF 編集、annotation、signature、OCR、text extraction を non-goal として固定する
- [ ] 1.4 v0.5.0 まで public CLI contract を固定しない方針を確認する

### Definition of Done

- [ ] PDF viewer rendering の目的と非目的が artifact に残っている
- [ ] export の置き換えではないことが明確である

---

## 2. Backend Selection

### 目的

PDF rendering backend を安全に選び、再現可能な初期化方法を固定する。

### タスク

- [ ] 2.1 PDFium / Poppler / MuPDF などの候補を比較する
- [ ] 2.2 license、配布方法、CI 再現性を確認する
- [ ] 2.3 backend binary / dynamic library の setup 手順を document 化する
- [ ] 2.4 backend missing 時の error を設計する

### Definition of Done

- [ ] backend の選定理由が design に追記されている
- [ ] backend がない環境で空 artifact を返さない

---

## 3. PDF Page Rendering

### 目的

PDF を page 単位の viewer artifact に変換する。

### タスク

- [ ] 3.1 `PdfRenderOptions` を追加する
- [ ] 3.2 page range、scale、background、max pages を実装する
- [ ] 3.3 page artifact と document metadata を返す
- [ ] 3.4 rotated page と crop box を扱う
- [ ] 3.5 render output の寸法を test で固定する

### Definition of Done

- [ ] 1 page / multi-page / rotated PDF を render できる
- [ ] metadata で page count と page size が分かる

---

## 4. Diagnostics

### 目的

PDF rendering の失敗を UI 非依存の診断として返す。

### タスク

- [ ] 4.1 backend missing diagnostic を実装する
- [ ] 4.2 invalid PDF diagnostic を実装する
- [ ] 4.3 password required diagnostic を実装する
- [ ] 4.4 unsupported feature diagnostic を実装する

### Definition of Done

- [ ] failure path が test で固定されている
- [ ] fallback で空 page artifact を返さない

---

## 5. Fixtures And Viewer E2E

### 目的

PDF viewer rendering の回帰と実表示を確認する。

### タスク

- [ ] 5.1 `tests/fixtures/pdf/` を追加する
- [ ] 5.2 normal、multi-page、rotated、large、invalid fixture を追加する
- [ ] 5.3 expected metadata と page artifact を検証する
- [ ] 5.4 viewer e2e に PDF case を追加する

### Definition of Done

- [ ] fixture が CI で再現可能である
- [ ] viewer e2e で PDF page artifact を表示できる

---

## 6. Final Verification

- [ ] 6.1 `just check` を実行する
- [ ] 6.2 PDF backend setup を含む CI smoke を確認する
- [ ] 6.3 PDF viewer e2e smoke を実行する
- [ ] 6.4 `npx -y @fission-ai/openspec validate v0-3-0-pdf-viewer-rendering --strict` を実行する
- [ ] 6.5 変更差分を自己レビューする
