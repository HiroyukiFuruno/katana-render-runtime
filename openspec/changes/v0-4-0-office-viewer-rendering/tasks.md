## 1. Office Rendering API

### Definition of Ready

- [ ] v0.2.0 CSV viewer rendering と v0.3.0 PDF viewer rendering の公開 API 境界を確認している
- [ ] Word / Excel / PPTX 以外を non-goal として扱う方針が固定されている
- [ ] KatanA UI state に依存しない入力と出力の型が決まっている

### 目的

Office viewer rendering の公開 API と error model を固定する。

### 書き込み範囲

- `crates/katana-canvas-forge`
- `openspec/changes/v0-4-0-office-viewer-rendering`

### タスク

- [ ] 1.1 Word / Excel / PPTX の入力 format を判定する型を追加する
- [ ] 1.2 viewer rendering result に artifact path、page / sheet / slide metadata、warning を含める
- [ ] 1.3 未対応 format、破損 file、password protected file、macro enabled file の error を型で表現する
- [ ] 1.4 KatanA 固有の workspace state、preview state、UI state を API に含めないことを確認する
- [ ] 1.5 API 境界の unit test を追加する

### Definition of Done

- [ ] Office rendering API が Word / Excel / PPTX に限定されている
- [ ] unsupported feature が暗黙 fallback になっていない
- [ ] KatanA consumer が artifact metadata を読める

---

## 2. Word / Excel / PPTX Rendering

### Definition of Ready

- [ ] 使用する rendering engine、外部 process、bundle、checksum、version pinning の扱いが決まっている
- [ ] 商用 Office アプリケーションを必須にしない方針が固定されている

### 目的

Office 文書を表示用 artifact に変換する。

### 書き込み範囲

- `crates/katana-canvas-forge`
- `vendor`
- `tests/fixtures`

### タスク

- [ ] 2.1 `.docx` の文章、見出し、表、画像を表示用 artifact に変換する
- [ ] 2.2 `.xlsx` の sheet、cell、style、画像を表示用 artifact に変換する
- [ ] 2.3 `.pptx` の slide、text、shape、image を表示用 artifact に変換する
- [ ] 2.4 embedded media、macro、password protected document の扱いを明示する
- [ ] 2.5 platform 差分が出る箇所を warning または test expectation として固定する
- [ ] 2.6 Excel の sheet / row / column range rendering を実装する

### Definition of Done

- [ ] Word / Excel / PPTX の最小 fixture が rendering できる
- [ ] 失敗時に原因が公開 error として返る
- [ ] rendering engine の version と checksum が固定されている

---

## 3. Viewer E2E And Reference Snapshots

### 目的

Office rendering の見た目を reference と比較できるようにする。

### 書き込み範囲

- `test/e2e/viewer`
- `tests/fixtures`
- `Justfile`

### タスク

- [ ] 3.1 Word / Excel / PPTX の最小 case と代表 case を追加する
- [ ] 3.2 reference snapshot 更新 recipe を追加する
- [ ] 3.3 表示確認 case で generated artifact と reference artifact を比較表示する
- [ ] 3.4 score report と screenshot report を保存する
- [ ] 3.5 KatanA consumer compatibility の smoke case を追加する
- [ ] 3.6 既知差分や score 改善候補を v0.4.x の候補として report に残す

### Definition of Done

- [ ] 表示確認 smoke が Office case を読み込める
- [ ] reference snapshot の更新方法が just recipe で再現できる
- [ ] KatanA 側で利用する artifact metadata が report に残る

---

## 4. Quality Gate

- [ ] 4.1 `cargo fmt --all -- --check` を実行する
- [ ] 4.2 `cargo clippy --workspace --all-targets -- -D warnings` を実行する
- [ ] 4.3 `cargo test --workspace` を実行する
- [ ] 4.4 Office viewer rendering smoke を実行する
- [ ] 4.5 `/lint-and-ast-lint` を実行し、静的検査（lint）と抽象構文木検査（AST lint）の結果を記録する
- [ ] 4.6 release gate を実行し、package 対象外の fixture や snapshot が混入していないことを確認する
- [ ] 4.7 `/self-review` を実行する
- [ ] 4.8 `npx -y @fission-ai/openspec validate v0-4-0-office-viewer-rendering --strict` を実行する

---

## 5. User Review

> ユーザーから受けた指摘は `[/]` で閉じる。通常の開発タスク `[x]` と混ぜない。

- [ ] 5.1 実装結果、表示確認結果、release gate 結果をユーザーに提示する
- [ ] 5.2 フィードバックを本 `tasks.md` に追記し、対応済みを `[/]` にする
