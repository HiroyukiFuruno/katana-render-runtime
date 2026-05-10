# Tasks: katana-canvas-forge v0.1.4 RenderInput theme application

## Branch Rule

- **標準ブランチ**: `release/v0.1.4`
- **作業ブランチ**: `feature/v0.1.4-task-x`

---

## User Feedback

- [/] Issue #4: KatanA から light を `RenderInput` に渡しても dark で描画される。kcf 側で `RenderInput` の theme 情報を実描画と cache fingerprint に反映する。

---

## 1. 公開 DTO への typed theme snapshot 追加

### 目的

`RenderContext` に typed な theme snapshot を追加し、consumer が theme 情報を type-safe に渡せるようにする。

### タスク

- [ ] 1.1 `RenderThemeMode` (Light / Dark) と `RenderThemeSnapshot` 型を `crates/katana-canvas-forge/src/renderer/api.rs` に追加する
- [ ] 1.2 `RenderContext` に `theme: Option<RenderThemeSnapshot>` field を追加する（Default / Serialize / Deserialize / Clone を維持）
- [ ] 1.3 `RenderThemeSnapshot` に含める field を `DiagramColorPreset` の描画影響 field に限定し、doc comment で対応関係を明示する
- [ ] 1.4 `RenderContext.theme` と `theme_fingerprint` の優先順位を doc comment で明示する

### Definition of Done

- [ ] 既存 consumer の build を壊さず、`RenderContext::default()` に `theme: None` が入る
- [ ] `RenderThemeSnapshot` が `Hash` / `Clone` / `Serialize` / `Deserialize` を満たす

---

## 2. DiagramColorPreset の owned 化と effective preset helper

### 目的

`RenderInput` 由来の snapshot から `DiagramColorPreset` を組み立てられるように、preset 内部表現を owned 対応にする。

### タスク

- [ ] 2.1 `DiagramColorPreset` の `&'static str` field を `Cow<'static, str>` に変更する（既存 static 値は `Cow::Borrowed`）
- [ ] 2.2 `Hash` / `Clone` / `Default` 派生と既存呼び出しが通ることを確認する
- [ ] 2.3 `crates/katana-canvas-forge/src/markdown/color_preset/` に `effective_preset(input: &RenderInput) -> DiagramColorPreset` を追加する
- [ ] 2.4 `effective_preset()` は `input.context.theme.is_some()` のとき snapshot から preset を構築し、それ以外は `DiagramColorPreset::current()` を fallback として返す
- [ ] 2.5 `effective_preset()` の単体テスト（snapshot 経由 / fallback の両 path）を追加する

### Definition of Done

- [ ] `DiagramColorPreset` が owned 値も保持できる
- [ ] `effective_preset()` が snapshot 優先 / global fallback の順で動作する

---

## 3. Mermaid / Draw.io renderer の preset 引数化

### 目的

renderer が `RenderInput` から組み立てた preset を描画層に渡し、`DiagramColorPreset::current()` の直接参照を撤去する。

### タスク

- [ ] 3.1 `MermaidRenderOps::render_mermaid_with_runtime_path()` の signature を `(block, mermaid_js, preset: &DiagramColorPreset)` に変更する
- [ ] 3.2 `crates/katana-canvas-forge/src/markdown/mermaid_renderer/render.rs` から `DiagramColorPreset::current()` 直接参照を撤去し、引数の preset を使う
- [ ] 3.3 `cache_file_path()` も引数の preset で hash するよう更新する
- [ ] 3.4 `DrawioRendererOps::render_drawio_with_runtime_path()` も同様に preset 引数を受け取るよう変更する
- [ ] 3.5 Draw.io 側の描画 / cache path も引数 preset を使うよう更新する
- [ ] 3.6 `MermaidRenderer::render_block()` / `DrawioRenderer::render_block()` で `effective_preset(input)` を呼び、描画層へ渡す
- [ ] 3.7 既存呼び出し元（scripts や internal helper）を新 signature に追従する

### Definition of Done

- [ ] renderer / 描画層から `DiagramColorPreset::current()` 直接参照が無くなる（fallback の effective_preset 内部のみ）
- [ ] `just check` と既存単体テストが通る

---

## 4. CacheFingerprintOps を effective theme 化

### 目的

cache fingerprint を `RenderInput` 由来の実効 theme で計算し、global state ではなく consumer 指定の theme 差分で fingerprint が変わるようにする。

### タスク

- [ ] 4.1 `CacheFingerprintOps::hash_current_theme()` を `hash_effective_theme(hasher, input)` に置き換える
- [ ] 4.2 `hash_effective_theme()` は `effective_preset(input)` の各 field を hash する
- [ ] 4.3 既存 test `render_fingerprint_changes_with_current_theme` を fallback path 用に維持する
- [ ] 4.4 新 test `render_fingerprint_changes_with_render_input_theme` を追加し、`theme` snapshot 差分で fingerprint が変わることを確認する

### Definition of Done

- [ ] fingerprint が `RenderInput.context.theme` の差分で確実に変わる
- [ ] global state fallback path も引き続き fingerprint を変える

---

## 5. 回帰テスト

### 目的

Issue #4 の症状（global が dark のとき light input が無視される）が再発しないことを保証する。

### タスク

- [ ] 5.1 `light_input_renders_with_light_preset_even_when_global_is_dark` テストを追加する
  - `DARK_MODE` を true に設定
  - `RenderInput.context.theme = Some(light)` で renderer を起動
  - 描画層に渡された preset が light 値であることを spy / fake runtime で確認
- [ ] 5.2 `light_and_dark_inputs_produce_distinct_outputs` テストを追加する
  - 同一 source で light / dark の `RenderInput` を render し、出力 SVG または描画 request が異なる
- [ ] 5.3 Mermaid / Draw.io の両 backend で 5.1 / 5.2 相当を網羅する
- [ ] 5.4 既存 fixture / score test が回帰しないことを確認する

### Definition of Done

- [ ] Issue #4 の再発を防ぐ test が backend 単位で存在する
- [ ] 既存 score / fixture test が回帰していない

---

## 6. Final Verification

- [ ] 6.1 `just check` を実行する
- [ ] 6.2 `just mermaid-compare-full` を実行する
- [ ] 6.3 `git diff --check` を実行する
- [ ] 6.4 PR 作成後に `@codex review` を依頼する
- [ ] 6.5 `npx -y @fission-ai/openspec validate "v0-1-4-render-input-theme-application" --strict` を実行する
- [ ] 6.6 KatanA v0.22.13 OpenSpec 側の adapter / 回帰テスト計画と整合していることを確認する
