## Why

Issue [#4](https://github.com/HiroyukiFuruno/katana-diagram-renderer/issues/4) で、KatanA から light テーマを `RenderInput` に渡しても、Mermaid / Draw.io が dark 配色で描画される事象が報告された。

原因は、kdr 側の renderer (`MermaidRenderer` / `DrawioRenderer`) が `RenderInput` の theme 関連情報を実描画に渡しておらず、`render_mermaid_with_runtime_path()` / `render_drawio_with_runtime_path()` が `DiagramColorPreset::current()` という process global な atomic state を直接参照していることにある。`DARK_MODE` の初期値は `true` のため、consumer が明示的に light を要求しても、global state が dark のままだと dark で描画される。

cache fingerprint も `DiagramColorPreset::current()` を hash しており、同一 source で consumer 指定の light / dark を切り替えても fingerprint が global state でしか変化しない。

v0.1.3 では、kdr 公開 DTO に typed theme snapshot を追加し、renderer は global state ではなく `RenderInput` から組み立てた実効テーマで描画と cache fingerprint を計算する。

## What Changes

- 公開 DTO `RenderContext` に typed な theme snapshot field（dark/light モード、`DiagramColorPreset` 相当の色情報）を追加する
- `MermaidRenderer::render_block()` / `DrawioRenderer::render_block()` が `RenderInput` から実効 `DiagramColorPreset` を組み立て、描画層に渡す
- `MermaidRenderOps::render_mermaid_with_runtime_path()` / `DrawioRendererOps::render_drawio_with_runtime_path()` の signature を、preset を引数で受け取る形へ変更する
- `DiagramColorPreset::current()` / `DARK_MODE` の global 参照は、`RenderInput` で theme snapshot が指定されなかった場合の fallback に限定する
- `CacheFingerprintOps::hash_current_theme()` を、`RenderInput` 由来の実効 theme を hash する `hash_effective_theme(input)` へ置き換える
- `RenderInput` で外部指定された light / dark の値が、runtime request / preset と cache fingerprint に反映されることを示す回帰テストを追加する
- `DARK_MODE` の global initial 値が true のままでも、light の `RenderInput` が light で描画されることを示すテストを追加する
- `release/v...` の取り込み依頼（Pull Request）作成前に、対象 version 以前の OpenSpec change が archive 済みであることを `lefthook run pre-pr` で確認する
- 通常の取り込み依頼（Pull Request）では full check を必ず実行し、`master` への merge push では release workflow だけを実行する
- `master` への直接 push では、動作影響がある差分だけ Rust full check を実行し、`*.md`、`scripts/`、`.github/` などの運用・文書差分では Rust full check を省く

## Non-Goals

- `DiagramColorPreset` の色 palette 自体を変更しない
- KatanA 側の `DiagramThemeSnapshot` 生成ロジックを変更しない（kdr 側の DTO に整合する形での adapter 更新は KatanA v0.22.13 OpenSpec で扱う）
- Draw.io / Mermaid の score 評価や画像類似度の再評価は行わない
- 不要になった export CSS debug 計画はKDR release番号から外し、旧 export/debug 論点は v0.1.5 のKDV移譲記録で扱う
- `DiagramColorPreset::current()` の global state そのものを撤去しない（fallback 用途で残す）

## Capabilities

### New Capabilities

- `render-input-theme-application`: `RenderInput` に含まれる consumer 指定の theme 情報を、Mermaid / Draw.io の実描画と cache fingerprint に必ず反映する

## Impact

- `crates/katana-diagram-renderer/src/renderer/api.rs` — `RenderContext` に typed theme snapshot field を追加
- `crates/katana-diagram-renderer/src/renderer/backends.rs` — `MermaidRenderer` / `DrawioRenderer` で `RenderInput` から実効 preset を組み立てる
- `crates/katana-diagram-renderer/src/renderer/fingerprint.rs` — `CacheFingerprintOps` が `RenderInput` 由来の実効 theme を hash する
- `crates/katana-diagram-renderer/src/markdown/mermaid_renderer/render.rs` — `DiagramColorPreset::current()` 直接参照を引数渡しに変更
- `crates/katana-diagram-renderer/src/markdown/drawio_renderer/` — Draw.io 側の同等修正
- `crates/katana-diagram-renderer/src/markdown/color_preset/` — `RenderInput` 由来 snapshot から `DiagramColorPreset` を組み立てる helper を追加
- `.github/workflows/ci.yml` / `lefthook.yml` / `scripts/release/` — release PR と master push の検査分岐、OpenSpec archive gate
- `openspec/changes/v0-1-3-render-input-theme-application/` — 本 change の仕様とタスク
