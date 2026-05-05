## ADDED Requirements

### Requirement: KatanA 側の既存能力を落とさない公開 API を提供しなければならない

システムは、KatanA 側でできていた Mermaid / Draw.io / export / score の能力を落とさない kcf 公開 API を提供しなければならない（MUST）。KatanA 側の型名を完全一致させること自体は要求しないが、kcf 側で独自に縮小、改名、簡略化した結果として KatanA の既存機能を失わせてはならない（MUST NOT）。

#### Scenario: KatanA consumer が kcf を利用する

- **WHEN** KatanA が kcf を git dependency として利用する
- **THEN** KatanA 側の adapter は型変換と呼び出し接続だけを担う
- **THEN** kcf の `RenderInput` / `RenderOutput` / `RenderError` / `ExportInput` / `ExportOutput` / `ExportError` は KatanA 側の既存呼び出しに必要な情報を保持する
- **THEN** kcf 独自都合の DTO 欠落や format 削減により KatanA 側の既存機能が落ちない

#### Scenario: kcf 側で API を整理する

- **WHEN** kcf が KatanA 側の `diagram_backend` 型名と異なる API を公開する
- **THEN** 既存 KatanA interface との差分を design / tasks に記録する
- **THEN** KatanA 側 adapter が必要とする情報を kcf 側の DTO で表現できる
- **THEN** KatanA 側で実装済みだった描画、書き出し、採点評価の機能差分を adapter 側で再実装させない

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
- **THEN** 取り込み version 固定、最新版確認、取り込み just recipe の整備は v0.1.2 の対象として分離する

### Requirement: runtime path は境界で非 null に解決しなければならない

システムは、最表層で未指定に仕様上の意味がある入力だけ nullable として受け取り、内部 renderer へ入る前に非 null 値へ解決しなければならない（MUST）。runtime path の未解決を renderer 内部の `Option` や暗黙 fallback として扱ってはならない（MUST NOT）。

#### Scenario: CLI で runtime path が明示される

- **WHEN** 利用者が `--runtime <path>` を指定する
- **THEN** システムは指定 path を `PathBuf` として renderer に渡す
- **THEN** renderer は `Option<PathBuf>` ではなく非 null の `PathBuf` を保持する

#### Scenario: CLI で runtime path が未指定である

- **WHEN** 利用者が `--runtime` を省略する
- **THEN** システムは `RuntimePathResolver` で同梱 runtime の path を解決する
- **THEN** 解決に失敗した場合は `RenderError::RuntimeResolution` として冒頭で失敗する
- **THEN** renderer 内部で既定 path、空 path、別 runtime へ暗黙 fallback しない

### Requirement: KatanA 既存 export runtime を忠実移植しなければならない

システムは、KatanA 既存の HTML / PDF / PNG / JPEG export を、機能を削らず kcf へ移植しなければならない（MUST）。export は `Exporter` trait と中立 DTO で公開し、KatanA 側の呼び出し元は実装 crate の詳細を知らない。

#### Scenario: HTML / PDF / PNG / JPEG export を Exporter trait 経由で提供する

- **WHEN** KatanA が `Exporter::export(&ExportInput)` を呼ぶ
- **THEN** システムは指定 format の出力ファイル path を返す
- **THEN** export 未対応経路は `ExportError::UnsupportedFormat` を返し、暗黙 fallback を持たない
- **THEN** HTML / PDF / PNG / JPEG の既存 KatanA 回帰テストと visual export test の検証観点を kcf 側で保持する

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

### Requirement: 公式 reference SVG / PNG は git 管理しなければならない

システムは、Mermaid / Draw.io の公式 reference SVG / PNG を repository 内で git 管理しなければならない（MUST）。CI/CD は公式 reference を外部から再取得・再生成してはならない（MUST NOT）。

#### Scenario: CI/CD で reference score を実行する

- **WHEN** CI/CD が Mermaid / Draw.io compare を実行する
- **THEN** git 管理済みの公式 reference SVG / PNG を読み込む
- **THEN** kcf 出力だけをその場で生成する
- **THEN** 公式 reference SVG / PNG を変更しない
- **THEN** 外部 network から公式描画結果を再取得しない

#### Scenario: 公式 reference を更新する

- **WHEN** 開発者が明示的に reference-update を実行する
- **THEN** 公式 reference SVG / PNG を再生成する
- **THEN** 再生成された SVG / PNG は git diff として review できる
- **THEN** CI/CD の通常 compare 経路とは分離される

### Requirement: reference score は評価階層を分離しなければならない

システムは、reference score を疎通確認（smoke check）、代表ケース評価（representative evaluation）、全量評価（full evaluation）に分離しなければならない（MUST）。`basic` fixture だけを vendor 互換性の保証として扱ってはならない（MUST NOT）。

#### Scenario: CI/CD で代表ケース評価を実行する

- **WHEN** CI/CD が Mermaid / Draw.io reference score を実行する
- **THEN** `representative` fixture と git 管理済み公式 reference SVG / PNG を使う
- **THEN** 公式 reference を再取得・再生成しない
- **THEN** Draw.io は basic shape、HTML label、layer、image、cloud stencil、UML、network、floor plan を含む
- **THEN** Draw.io の既知差分は `score-baseline.json` の下限を使い、現在値からの悪化を検知する

#### Scenario: ローカルで全量評価を実行する

- **WHEN** 開発者が release validation として full compare を実行する
- **THEN** Mermaid は KatanA から移植した full fixture を比較する
- **THEN** Draw.io は `basic` と `official` 配下の全カテゴリを比較する
- **THEN** git 管理済み公式 reference SVG / PNG と kcf 出力だけを比較する
- **THEN** score 改善対象は v0.1.1 の作業として report に残す

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

### Requirement: coverage gate は未到達行 0 を要求しなければならない

システムは、v0.1.0 の品質ゲートとして行カバレッジ（line coverage）100%、未到達行（uncovered line）0 を要求しなければならない（MUST）。coverage を上げるために不要な fallback や dead logic を残してはならない（MUST NOT）。

#### Scenario: release check を実行する

- **WHEN** `just coverage` または `just VERSION=v0.1.0 release-check` を実行する
- **THEN** `cargo llvm-cov` は `--fail-under-lines 100` と `--fail-uncovered-lines 0` を使う
- **THEN** 未到達行が 1 行でも残る場合は失敗する
- **THEN** 不要な分岐はテストで覆うのではなく削除し、必要な失敗経路は error first のテストで覆う
