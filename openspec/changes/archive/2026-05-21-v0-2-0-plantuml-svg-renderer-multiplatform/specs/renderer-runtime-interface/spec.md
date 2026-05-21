## MODIFIED Requirements

### Requirement: KatanA 側の既存能力を落とさない公開 API を提供しなければならない

システムは、KatanA 側でできていた Mermaid / Draw.io / PlantUML / score の能力を落とさない kdr 公開 API を提供しなければならない（MUST）。KatanA 側の型名を完全一致させること自体は要求しないが、kdr 側で独自に縮小、改名、簡略化した結果として KatanA の既存描画機能を失わせてはならない（MUST NOT）。

#### Scenario: KatanA consumer が kdr を利用する

- **WHEN** KatanA が kdr を git dependency として利用する
- **THEN** KatanA 側の adapter は型変換と呼び出し接続だけを担う
- **THEN** kdr の `RenderInput` / `RenderOutput` / `RenderError` は KatanA 側の既存呼び出しに必要な情報を保持する
- **THEN** kdr 独自都合の DTO 欠落や format 削減により KatanA 側の Mermaid / Draw.io / PlantUML 既存機能が落ちない

#### Scenario: kdr 側で API を整理する

- **WHEN** kdr が KatanA 側の `diagram_backend` 型名と異なる API を公開する
- **THEN** 既存 KatanA interface との差分を design / tasks に記録する
- **THEN** KatanA 側 adapter が必要とする情報を kdr 側の DTO で表現できる
- **THEN** KatanA 側で実装済みだった描画、採点評価、PlantUML process 診断の機能差分を adapter 側で再実装させない

### Requirement: KatanA 既存描画 runtime を忠実移植しなければならない

システムは、KatanA 既存の Mermaid / Draw.io / PlantUML 描画 runtime を、機能を削らず kdr へ移植しなければならない（MUST）。移植時は KatanA UI state と KatanA 固有 path 前提を剥がし、`Renderer` trait と中立 DTO（`RenderInput` / `RenderOutput` / `RenderConfig` / `RenderPolicy` / `RenderContext` / `RenderDiagnostics` / `RuntimeVersion` / `RendererProfile`）で公開しなければならない。

#### Scenario: KatanA から Mermaid を描画する

- **WHEN** KatanA が `Renderer::render(&RenderInput)` を Mermaid backend で呼ぶ
- **THEN** システムは `RenderOutput`（SVG・幅・高さ・viewBox・`RuntimeVersion`・`RendererProfile`・`RenderDiagnostics`・`cache_fingerprint`）を返す
- **THEN** vendor 互換 config は `RenderConfig`、KatanA 独自制約は `RenderPolicy` として分離される
- **THEN** KatanA 既存の Mermaid runtime script、normalizer、theme、i18n、fixture coverage と同等の結果を保持する

#### Scenario: KatanA から Draw.io を描画する

- **WHEN** KatanA が `Renderer::render(&RenderInput)` を Draw.io backend で呼ぶ
- **THEN** システムは KatanA 既存 Draw.io runtime と同等の SVG とメタデータを返す
- **THEN** Draw.io resource、stencil、image resolver は kdr 側の vendor/resource 管理に移される
- **THEN** KatanA UI state、preview state、workspace state への依存は持たない

#### Scenario: KatanA から PlantUML を描画する

- **WHEN** KatanA または KDV が `Renderer::render(&RenderInput)` を PlantUML backend で呼ぶ
- **THEN** システムは KatanA 既存 PlantUML renderer と同等の SVG、または runtime 不足時の raw code block とメタデータを返す
- **THEN** Java / `plantuml.jar` の解決と失敗診断は kdr 側で行う
- **THEN** runtime 不足時の診断は CLI と公開 API で共通の warning code、原因、確認 path、install / env 設定の対処を持つ
- **THEN** KatanA UI state、preview state、workspace state、KDV export state への依存は持たない

#### Scenario: runtime asset を移植する

- **WHEN** kdr が Mermaid / Draw.io / PlantUML runtime を初期化する
- **THEN** KatanA 既存 runtime が必要とする Mermaid.js / Draw.io.js / PlantUML asset を kdr 側で読み込める
- **THEN** 取り込み version 固定、最新版確認、取り込み just recipe の整備は runtime asset versioning の対象として分離する
