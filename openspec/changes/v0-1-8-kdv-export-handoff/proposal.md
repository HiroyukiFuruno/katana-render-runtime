## Why

旧 export/debug 実装 branch には、HTML / PDF / PNG / JPG export、README相対パス解決、macOS debug open、利用側Rust GUI品質ゲートが含まれていた。

設計整理により、KDR は Mermaid / Draw.io の図形描画（rendering）と reference score に専念する。viewer、document export、CSV、PDF、Office は KDV（katana-document-viewer）へ移譲する。

v0.1.7 では、旧 export/debug 実装 branch の内容をKDRへ再取り込みしない。KDVへ渡すべき論点だけを記録し、KDR側には export や viewer rendering 依存を増やさない。

## What Changes

- 旧 export/debug 実装 branch の内容をKDV移譲対象として棚卸しする
- HTML / PDF / PNG / JPG export は KDV v0.1.0 の viewer/export pipeline へ移譲する
- README相対パス解決、file path付き入力、macOS debug open はKDV側のexport品質ゲート候補として扱う
- CSV / PDF / Office viewer は KDV v0.2.0 以降のmulti-format viewerへ移譲する
- KDR側には Mermaid / Draw.io rendering に必要な外部描画APIだけを残す
- 旧 export/debug 実装 branch は実装sourceではなく、移譲判断の履歴として扱う

## Non-Goals

- 旧 export/debug 実装 branch のexport実装をKDR masterへmergeしない
- `kdr-e2e-host` をKDRの正式品質ゲートとして再導入しない
- KDRでHTML / PDF / PNG / JPG exportを拡張しない
- KDRでCSV / PDF / Office viewerを実装しない
- KDVのv0.1.0実装をこのchangeで行わない

## Capabilities

### New Capabilities

- `kdv-export-handoff`: KDR旧export/debug品質ゲートのうち、KDVへ渡す論点を明文化する
- `kdr-rendering-scope-lock`: KDRの責務をMermaid / Draw.io renderingへ固定する

## Impact

- `openspec/changes/v0-1-8-kdv-export-handoff/` — KDV移譲記録
- `katana-document-viewer` — HTML / PDF / PNG / JPG export、CSV / PDF / Office viewerの移譲先
- 旧 export/debug 実装 branch — 参照用の履歴。KDR masterへそのまま取り込まない
