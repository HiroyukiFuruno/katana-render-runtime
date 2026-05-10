## Why

Issue [#4](https://github.com/HiroyukiFuruno/katana-canvas-forge/issues/4) で、KatanA から light テーマを `RenderInput` に渡しても、Mermaid / Draw.io が dark 配色で描画される事象が報告された。

原因は、kcf 側の renderer (`MermaidRenderer` / `DrawioRenderer`) が `RenderInput` の theme 関連情報を実描画に渡しておらず、`render_mermaid_with_runtime_path()` / `render_drawio_with_runtime_path()` が `DiagramColorPreset::current()` という process global な atomic state を直接参照していることにある。`DARK_MODE` の初期値は `true` のため、consumer が明示的に light を要求しても、global state が dark のままだと dark で描画される。

cache fingerprint も `DiagramColorPreset::current()` を hash しており、同一 source で consumer 指定の light / dark を切り替えても fingerprint が global state でしか変化しない。

v0.1.4 では、kcf 公開 DTO に typed theme snapshot を追加し、renderer は global state ではなく `RenderInput` から組み立てた実効テーマで描画と cache fingerprint を計算する。

## What Changes

- 公開 DTO `RenderContext` に typed な theme snapshot field（dark/light モード、`DiagramColorPreset` 相当の色情報）を追加する
- `MermaidRenderer::render_block()` / `DrawioRenderer::render_block()` が `RenderInput` から実効 `DiagramColorPreset` を組み立て、描画層に渡す
- `MermaidRenderOps::render_mermaid_with_runtime_path()` / `DrawioRendererOps::render_drawio_with_runtime_path()` の signature を、preset を引数で受け取る形へ変更する
- `DiagramColorPreset::current()` / `DARK_MODE` の global 参照は、`RenderInput` で theme snapshot が指定されなかった場合の fallback に限定する
- `CacheFingerprintOps::hash_current_theme()` を、`RenderInput` 由来の実効 theme を hash する `hash_effective_theme(input)` へ置き換える
- 同一 source で light / dark の `RenderInput` を渡した場合に、renderer の出力と cache fingerprint が異なることを示す回帰テストを追加する
- `DARK_MODE` の global initial 値が true のままでも、light の `RenderInput` が light で描画されることを示すテストを追加する

## Non-Goals

- `DiagramColorPreset` の色 palette 自体を変更しない
- KatanA 側の `DiagramThemeSnapshot` 生成ロジックを変更しない（kcf 側の DTO に整合する形での adapter 更新は KatanA v0.22.13 OpenSpec で扱う）
- v0.1.5 の Draw.io / Mermaid score 改善に混ぜない
- export CSS 回帰修正と macOS debug open は KDV 移譲対象として v0.1.6 に送る
- `DiagramColorPreset::current()` の global state そのものを撤去しない（fallback 用途で残す）

## Capabilities

### New Capabilities

- `render-input-theme-application`: `RenderInput` に含まれる consumer 指定の theme 情報を、Mermaid / Draw.io の実描画と cache fingerprint に必ず反映する

## Impact

- `crates/katana-canvas-forge/src/renderer/api.rs` — `RenderContext` に typed theme snapshot field を追加
- `crates/katana-canvas-forge/src/renderer/backends.rs` — `MermaidRenderer` / `DrawioRenderer` で `RenderInput` から実効 preset を組み立てる
- `crates/katana-canvas-forge/src/renderer/fingerprint.rs` — `CacheFingerprintOps` が `RenderInput` 由来の実効 theme を hash する
- `crates/katana-canvas-forge/src/markdown/mermaid_renderer/render.rs` — `DiagramColorPreset::current()` 直接参照を引数渡しに変更
- `crates/katana-canvas-forge/src/markdown/drawio_renderer/` — Draw.io 側の同等修正
- `crates/katana-canvas-forge/src/markdown/color_preset/` — `RenderInput` 由来 snapshot から `DiagramColorPreset` を組み立てる helper を追加
- `openspec/changes/v0-1-4-render-input-theme-application/` — 本 change の仕様とタスク
