## ADDED Requirements

### Requirement: KatanA 側で定義済みの interface を完全踏襲しなければならない

システムは、KatanA 側で定義済みの renderer / exporter interface と中立 DTO を、v0.1.0 transfer の正本として完全踏襲しなければならない（MUST）。kcf 側で独自に縮小、改名、簡略化した contract に置き換えてはならない（MUST NOT）。

#### Scenario: KatanA consumer が kcf を利用する

- **WHEN** KatanA が kcf を git dependency として利用する
- **THEN** KatanA 側で定義済みの renderer / exporter 呼び出し contract が維持される
- **THEN** KatanA 側の adapter は実装移管のための薄い接続だけを担う
- **THEN** kcf 独自都合の DTO 欠落や format 削減により KatanA 側の既存機能が落ちない

#### Scenario: interface 変更が必要に見える

- **WHEN** 移植中に interface の不足が見つかる
- **THEN** 既存 KatanA interface との差分を design / tasks に記録する
- **THEN** 実装者判断で kcf 側に簡略 contract を作らない

### Requirement: KatanA 既存描画 runtime を忠実移植しなければならない

システムは、KatanA 既存の Mermaid / Draw.io 描画 runtime を、機能を削らず kcf へ移植しなければならない（MUST）。移植時は KatanA UI state と KatanA 固有 path 前提を剥がし、`Renderer` trait と中立 DTO（`RenderInput` / `RenderOutput` / `RenderConfig` / `RenderPolicy` / `RenderContext` / `RenderDiagnostics` / `RuntimeVersion` / `RendererProfile`）で公開しなければならない。

#### Scenario: KatanA から Mermaid を描画する

- **WHEN** KatanA が `Renderer::render(&RenderInput)` を Mermaid backend で呼ぶ
- **THEN** システムは `RenderOutput`（SVG・幅・高さ・viewBox・`RuntimeVersion`・`RendererProfile`・`RenderDiagnostics`・`cache_fingerprint`）を返す
- **THEN** vendor 互換 config は `RenderConfig`、KatanA 独自制約は `RenderPolicy` として分離される
- **THEN** KatanA 既存の Mermaid runtime script、normalizer、theme、i18n、fixture coverage と同等の結果を保持する

#### Scenario: KatanA から Draw.io を描画する

- **WHEN** KatanA が `Renderer::render(&RenderInput)` を Draw.io backend で呼ぶ
- **THEN** システムは KatanA 既存 Draw.io runtime と同等の SVG とメタデータを返す
- **THEN** Draw.io resource、stencil、image resolver は kcf 側の vendor/resource 管理に移される
- **THEN** KatanA UI state、preview state、workspace state への依存は持たない

#### Scenario: runtime asset を移植する

- **WHEN** kcf が Mermaid / Draw.io runtime を初期化する
- **THEN** KatanA 既存 runtime が必要とする Mermaid.js / Draw.io.js asset を kcf 側で読み込める
- **THEN** 取り込み version 固定、最新版確認、取り込み just recipe の整備は v0.1.1 の対象として分離する

### Requirement: KatanA 既存 export runtime を忠実移植しなければならない

システムは、KatanA 既存の HTML / PDF / PNG / JPEG export を、機能を削らず kcf へ移植しなければならない（MUST）。export は `Exporter` trait と中立 DTO で公開し、KatanA 側の呼び出し元は実装 crate の詳細を知らない。

#### Scenario: HTML / PDF / PNG / JPEG export を Exporter trait 経由で提供する

- **WHEN** KatanA が `Exporter::export(&ExportInput)` を呼ぶ
- **THEN** システムは指定 format の出力ファイル path を返す
- **THEN** export 未対応経路は `ExportError::UnsupportedFormat` を返し、暗黙 fallback を持たない
- **THEN** HTML / PDF / PNG / JPEG の既存 KatanA 回帰テストと visual export test を kcf 側で保持する

### Requirement: Mermaid / Draw.io の reference 採点評価を移植しなければならない

システムは、KatanA 既存の Mermaid / Draw.io reference 生成と ImageMagick 採点評価を kcf へ移植しなければならない（MUST）。採点は SVG 文字列距離だけで代替してはならない。

#### Scenario: Mermaid reference と kcf 出力を採点する

- **WHEN** `kcf mermaid compare --min-score <score>` を実行する
- **THEN** 公式 Mermaid.js reference SVG / PNG と kcf が出力した SVG / PNG を生成する
- **THEN** ImageMagick による canvas / content の画像比較 score を出力する
- **THEN** 既存 baseline exception と score floor policy を保持する

#### Scenario: Draw.io reference と kcf 出力を採点する

- **WHEN** `kcf drawio compare --min-score <score>` を実行する
- **THEN** 公式 Draw.io reference SVG / PNG と kcf が出力した SVG / PNG を生成する
- **THEN** ImageMagick による RMSE / MAE / PHASH / dimension coverage を使った score を出力する
- **THEN** resource 解決と crop 正規化は KatanA 既存方式と同等に扱う

#### Scenario: kcf に egui / KatanA UI 依存を持たせない

- **WHEN** `cargo tree --workspace` を実行する
- **THEN** kcf workspace dependency graph に `egui`、KatanA UI state は含まれない
- **THEN** 描画結果は SVG 文字列とメタデータ（DTO）として返される

### Requirement: document viewer rendering 拡張を transfer から分離しなければならない

システムは、CSV / PDF / Word / Excel / PPTX を viewer 用に render する拡張要件を、v0.1.0 transfer から分離しなければならない（MUST）。

#### Scenario: v0.1.0 の完了条件を判定する

- **WHEN** v0.1.0 の完了可否を確認する
- **THEN** Mermaid / Draw.io / HTML / PDF / PNG / JPEG export / scoring の KatanA 既存実装移植を必須条件にする
- **THEN** CSV / PDF / Word / Excel / PPTX viewer rendering は後続 extension change の対象として扱う
