## ADDED Requirements

### Requirement: Renderer trait と中立 DTO で描画契約を提供しなければならない

システムは、Mermaid / Draw.io 描画と HTML / PDF / PNG / JPEG export の契約を、`Renderer` trait と中立 DTO（`RenderInput` / `RenderOutput` / `RenderConfig` / `RenderPolicy` / `RenderContext` / `RenderDiagnostics` / `RuntimeVersion` / `RendererProfile`）として提供しなければならない（MUST）。

#### Scenario: KatanA から Mermaid を描画する

- **WHEN** KatanA が `Renderer::render(&RenderInput)` を Mermaid backend で呼ぶ
- **THEN** システムは `RenderOutput`（SVG・幅・高さ・viewBox・`RuntimeVersion`・`RendererProfile`・`RenderDiagnostics`・`cache_fingerprint`）を返す
- **THEN** vendor 互換 config は `RenderConfig`、KatanA 独自制約は `RenderPolicy` として分離される

#### Scenario: Mermaid.js bundle を repository 内で版固定する

- **WHEN** kcf が Mermaid runtime を初期化する
- **THEN** `vendor/mermaid/<version>/mermaid.min.js` + `.sha256` から固定版を読み込む
- **THEN** 実行時に CDN / npm install / OS Chrome / Chromium app への依存はない

#### Scenario: HTML / PDF / PNG / JPEG export を Exporter trait 経由で提供する

- **WHEN** KatanA が `Exporter::export(&ExportInput)` を呼ぶ
- **THEN** システムは指定 format の出力ファイル path を返す
- **THEN** export 未対応経路は `ExportError::UnsupportedFormat` を返し、暗黙 fallback を持たない

#### Scenario: kcf に egui / KatanA UI 依存を持たせない

- **WHEN** `cargo tree --workspace` を実行する
- **THEN** kcf workspace dependency graph に `egui`、KatanA UI state は含まれない
- **THEN** 描画結果は SVG 文字列とメタデータ（DTO）として返される
