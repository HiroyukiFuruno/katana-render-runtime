## Why

旧 `release/v0.1.3` には、HTML / PDF / PNG / JPG export、README相対パス解決、macOS debug open、利用側Rust GUI品質ゲートが含まれていた。

設計整理により、KCF は Mermaid / Draw.io の図形描画（rendering）と reference score に専念する。viewer、document export、CSV、PDF、Office は KDV（katana-document-viewer）へ移譲する。

v0.1.6 では、旧 `release/v0.1.3` の内容をKCFへ再取り込みしない。KDVへ渡すべき論点だけを記録し、KCF側には export や viewer rendering 依存を増やさない。

## What Changes

- 旧 `release/v0.1.3` の内容をKDV移譲対象として棚卸しする
- HTML / PDF / PNG / JPG export は KDV v0.1.0 の viewer/export pipeline へ移譲する
- README相対パス解決、file path付き入力、macOS debug open はKDV側のexport品質ゲート候補として扱う
- CSV / PDF / Office viewer は KDV v0.2.0 以降のmulti-format viewerへ移譲する
- KCF側には Mermaid / Draw.io rendering に必要な外部描画APIだけを残す
- 旧 `release/v0.1.3` branch は実装sourceではなく、移譲判断の履歴として扱う

## Non-Goals

- 旧 `release/v0.1.3` のexport実装をKCF masterへmergeしない
- `kcf-e2e-host` をKCFの正式品質ゲートとして再導入しない
- KCFでHTML / PDF / PNG / JPG exportを拡張しない
- KCFでCSV / PDF / Office viewerを実装しない
- KDVのv0.1.0実装をこのchangeで行わない

## Capabilities

### New Capabilities

- `kdv-export-handoff`: KCF旧export/debug品質ゲートのうち、KDVへ渡す論点を明文化する
- `kcf-rendering-scope-lock`: KCFの責務をMermaid / Draw.io renderingへ固定する

## Impact

- `openspec/changes/v0-1-6-kdv-export-handoff/` — KDV移譲記録
- `katana-document-viewer` — HTML / PDF / PNG / JPG export、CSV / PDF / Office viewerの移譲先
- `release/v0.1.3` — 参照用の履歴branch。KCF masterへそのまま取り込まない
