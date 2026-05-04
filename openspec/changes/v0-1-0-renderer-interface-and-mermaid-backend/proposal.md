## Why

KatanA から Mermaid 描画責務を kcf へ移管する。kcf はまず Mermaid rendering の interface と backend を確立し、KatanA が git dependency として利用できる状態にする。これにより KatanA の検証範囲から描画実装が除外され、リリースごとの負荷が下がる。

## What Changes

- `Renderer` trait と中立 DTO（`RenderInput` / `RenderOutput` / `RenderConfig` / `RenderPolicy` / `RenderContext` / `RenderDiagnostics` / `RuntimeVersion` / `RendererProfile`）を確定する
- KatanA `crates/katana-core/src/markdown/mermaid_renderer/` の実装を kcf 側へ移管する
- `vendor/mermaid/<version>/mermaid.min.js` + `.sha256` で Mermaid.js を版固定管理する
- `just VERSION=<version> mermaid-js-update` で版更新・checksum・比較画像を一括更新する
- KatanA `assets/fixtures/mermaid_all/`、`scripts/mermaid/`、関連 test を kcf 側へ移管する
- `kcf mermaid render / reference-update / compare / bench` CLI を本実装にする
- `egui` / KatanA UI state が `cargo tree` に含まれないことを確認する
- `v0.1.0` として release tag を切る

## Capabilities

### New Capabilities

- `renderer-runtime-interface`: Mermaid rendering contract、versioned Mermaid.js bundle、reference scoring

## Impact

- `crates/katana-canvas-forge/src/renderer/` — trait + DTO 確定
- `crates/katana-canvas-forge/src/mermaid/` — KatanA から移管した Mermaid backend
- `crates/katana-canvas-forge-cli/src/` — CLI 実装
- `vendor/mermaid/<version>/` — pinned bundle
- `tests/fixtures/mermaid_all/` — KatanA から移管した fixtures
- `docs/` — 移管手順・採点方針
