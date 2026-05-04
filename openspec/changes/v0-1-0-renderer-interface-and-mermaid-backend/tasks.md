# Tasks: katana-canvas-forge v0.1.0

## Branch Rule

- **標準ブランチ**: `release/v0.1.0`
- **作業ブランチ**: `feature/v0.1.0-task-x`

---

## 1. Renderer trait と DTO を本実装にする

- [ ] 1.1 `crates/katana-canvas-forge/src/renderer.rs` の DTO（`RenderInput` / `RenderOutput` / `RenderConfig` / `RenderPolicy` / `RenderContext` / `RenderDiagnostics` / `RuntimeVersion` / `RendererProfile`）を本実装向けに拡張する
- [ ] 1.2 `Renderer` trait の単体テスト（mock 実装）を追加する
- [ ] 1.3 `katana-canvas-forge`（neutral interface crate）の `cargo tree` に `egui` が含まれないことを確認する
- [ ] 1.4 KatanA が interface crate のみを依存しても型エラーが出ないことを確認する

---

## 2. Mermaid backend と版固定を実装する

### 準備完了条件（Definition of Ready）

- [ ] Task 1 完了

### 実施内容

KatanA `crates/katana-core/src/markdown/mermaid_renderer/` の実装と Mermaid.js bundle 管理、checksum、更新 recipe を kcf 側へ移管する。

- [ ] 2.1 KatanA から移管した Mermaid 描画実装を `crates/katana-canvas-forge/src/mermaid/` に本実装する
- [ ] 2.2 `vendor/mermaid/<version>/mermaid.min.js` + `.sha256` で Mermaid.js を版固定管理する
- [ ] 2.3 `just VERSION=<version> mermaid-js-update` 相当を kcf 内で完結させる（版更新・checksum・比較画像一括更新）
- [ ] 2.4 `tests/fixtures/mermaid_all/` として fixture を管理する
- [ ] 2.5 Mermaid backend の unit test を追加する

---

## 3. kcf CLI を実装する

### 準備完了条件（Definition of Ready）

- [ ] Task 2 完了

- [ ] 3.1 `kcf mermaid render` を実装する
- [ ] 3.2 `kcf mermaid reference-update` を実装する（公式比較画像の生成）
- [ ] 3.3 `kcf mermaid compare --min-score 99` を実装する（採点評価）
- [ ] 3.4 `kcf mermaid bench` を実装する

---

## 4. CI/CD と採点評価を運用に乗せる

### 準備完了条件（Definition of Ready）

- [ ] Task 3 完了

- [ ] 4.1 `.github/workflows/` で fmt / clippy / test / 採点評価 job を有効化する
- [ ] 4.2 lefthook または equivalent で pre-commit 採点 hook を有効化する
- [ ] 4.3 `cargo fmt` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` が通る

---

## 5. v0.1.0 release

### 準備完了条件（Definition of Ready）

- [ ] Task 4 完了

- [ ] 5.1 release tag `v0.1.0` を切り GitHub Release を作成する
- [ ] 5.2 KatanA v0.22.11 が `katana-canvas-forge = { git = "...", tag = "v0.1.0" }` でビルドできることを確認する
