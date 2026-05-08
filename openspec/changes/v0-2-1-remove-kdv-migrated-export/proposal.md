## Why

KDVがHTML / PDF / PNG / JPG exportを担うと、KCFに残る既存export系機能は責務重複になる。

KCFはMermaid / Draw.ioの図形描画（rendering）とreference scoreへ集中する。KDV実装完了後、KCF側のdocument export機能を削除し、KDVがKCFの外部図形描画APIだけを呼び出す形へ整理する。

## What Changes

- KDV実装完了を開始条件として、KCF側のHTML / PDF / PNG / JPG exportを削除する
- KCF CLIのexport commandを削除または非公開化する
- KCF library APIからdocument export専用型と処理を削除する
- KDVが必要とするMermaid / Draw.io rendering APIは維持する
- README / docs / release gateからKCF exportの説明を削除する

## Non-Goals

- KDV側のexport実装をこのchangeで行わない
- Mermaid / Draw.io rendering APIを削除しない
- reference scoreやfull compareを削除しない
- KatanA固有のviewer stateをKCFへ追加しない

## Capabilities

### Removed Capabilities

- `kcf-document-export`: KCF側のHTML / PDF / PNG / JPG document export
- `kcf-export-cli`: KCF CLIのdocument export command

### Preserved Capabilities

- `mermaid-rendering`
- `drawio-rendering`
- `reference-score`

## Impact

- `crates/katana-canvas-forge/src/markdown/export/` — 削除またはrendering APIに必要な部分だけへ縮小
- `crates/katana-canvas-forge-cli/` — export command削除
- `README.md` / `docs` — KCF export説明削除、KDVへの誘導追加
- `katana-document-viewer` — document exportのowner
- `openspec/changes/v0-2-1-remove-kdv-migrated-export/` — 本changeの仕様とタスク
