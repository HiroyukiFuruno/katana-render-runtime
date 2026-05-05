## Why

KatanA から Mermaid / Draw.io / export / 採点評価の責務を kcf へ移管する。kcf v0.1.0 は新規の簡略実装ではなく、KatanA 既存 interface と実装の忠実移植（transfer）を目的とする。これにより KatanA は git dependency として kcf を利用し、描画・書き出し・比較検証の実装を KatanA 本体から切り離せる。

PR #1 は v0.1.0 の土台にしない。PR #1 は close 済みで、レビュー履歴だけを失敗パターンと論点整理の参照資料として扱う。

## What Changes

- KatanA 側で定義済みの `Renderer` / `Exporter` interface と中立 DTO を完全踏襲する
- interface は kcf 側で独自に縮小、改名、簡略化しない
- KatanA `crates/katana-core/src/markdown/mermaid_renderer/` の実装を kcf 側へ移管する
- KatanA `crates/katana-core/src/markdown/drawio_renderer/` の実装と resource 一式を kcf 側へ移管する
- KatanA `crates/katana-core/src/markdown/export/` の HTML / PDF / PNG / JPEG export 実装を kcf 側へ移管する
- KatanA `scripts/mermaid/`、`scripts/drawio/`、`assets/fixtures/mermaid_parts/`、`assets/fixtures/drawio/`、関連 test を kcf 側へ移管する
- Mermaid / Draw.io の公式 reference SVG / PNG 生成と ImageMagick 採点評価を kcf CLI と CI に組み込む
- `kcf mermaid ...` / `kcf drawio ...` / `kcf export ...` CLI を library の薄い利用者として本実装にする
- `egui` / KatanA UI state が `cargo tree` に含まれないことを確認する
- `v0.1.0` として release tag を切る

## Non-Goals

- PR #1 の簡略 Mermaid 実装、HTML のみ export、SVG 文字列比較 score を土台にしない
- KatanA 既存実装より機能を削った MVP を v0.1.0 完了扱いにしない
- KatanA 側で定義済みの interface から外れた kcf 独自 contract を新設しない
- Mermaid.js / Draw.io.js の取り込み version 固定、最新版確認、取り込み just recipe は v0.1.1 で扱う
- CSV / PDF / Word / Excel / PPTX viewer rendering は v0.1.0 transfer には含めない

## Capabilities

### New Capabilities

- `renderer-runtime-interface`: Mermaid / Draw.io rendering contract、runtime asset loading、reference scoring
- `exporter-interface`: HTML / PDF / PNG / JPEG export の `Exporter` 実装
- `reference-scoring`: Mermaid / Draw.io の公式 reference 生成、画像正規化、ImageMagick 採点評価

## Impact

- `crates/katana-canvas-forge/src/renderer/` — trait + DTO 確定
- `crates/katana-canvas-forge/src/exporter/` — HTML / PDF / PNG / JPEG export 実装
- `crates/katana-canvas-forge/src/mermaid/` — KatanA から移管した Mermaid backend
- `crates/katana-canvas-forge/src/drawio/` — KatanA から移管した Draw.io backend
- `crates/katana-canvas-forge-cli/src/` — CLI 実装
- `vendor/mermaid/` / `vendor/drawio/` — v0.1.0 transfer で必要な runtime asset 配置
- `tests/fixtures/mermaid/` / `tests/fixtures/drawio/` — KatanA から移管した fixtures
- `tests/reference/` — reference image generation and scoring tests
- `docs/` — 移管手順・採点方針
