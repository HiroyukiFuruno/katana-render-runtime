## Context

v0.2.0 は、v0.1.x の後続で最初に追加する document viewer rendering である。

ここでいう viewer rendering は、外部ファイルへの書き出し（export）ではない。既存ファイルを読み、画面表示に渡しやすい artifact に変換する処理である。

## Goals

- CSV を見やすい table artifact に変換する
- KatanA UI に依存しない library API にする
- 文字コード、区切り文字、引用符、複数行 field、header 有無を扱う
- 大きな CSV を安全に扱うため、行列範囲を指定できる
- 表示確認 case で実表示を確認できる

## Non-Goals

- spreadsheet engine を作らない
- CSV の値を勝手に数値、日付、真偽値へ変換しない
- CSV 編集、保存、再整形をしない
- v0.5.0 より前に public CLI contract を固定しない

## Public Boundary

v0.2.0 では、library 側に viewer rendering 用の中立 DTO を追加する。

- `ViewerRenderInput`: source path または bytes、format、render options
- `ViewerRenderOptions`: range、theme、max row、max column、encoding hint、delimiter hint
- `ViewerRenderOutput`: artifact kind、HTML fragment、metadata、diagnostics
- `ViewerArtifactMetadata`: row count、column count、header presence、encoding、delimiter、truncation

KatanA 側はこの DTO を受け取り、UI 表示だけを担当する。kcf は KatanA の tab、preview、workspace、selection state を知らない。

## CSV Rendering

CSV は parser で構造として読む。

- 区切り文字は hint があれば優先し、なければ候補から推定する
- 引用符付き field と field 内改行を保持する
- header は option で明示でき、未指定時は metadata として推定結果を返す
- 文字コードは UTF-8 を基本にし、非 UTF-8 は明示的な診断を返す
- 値の型変換は行わず、表示用 escape だけを行う

## Windowing

巨大 CSV を一度に全量 render しない。

- 既定では先頭範囲だけを render する
- `row_offset`、`row_limit`、`column_offset`、`column_limit` を指定できる
- 出力 metadata に省略された行数、列数、truncation reason を含める

## Viewer Artifact

初期 artifact は HTML fragment とする。

HTML fragment は table、thead、tbody、cell class、data attribute だけを持ち、script を含めない。style は theme token か最小 CSS class に留める。

## Verification

- parser unit test
- fixture based integration test
- large CSV の memory / time smoke
- 表示確認 screenshot
- `just check`
- `npx -y @fission-ai/openspec validate v0-2-0-csv-viewer-rendering --strict`
