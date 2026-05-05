## Why

v0.1.0 で Mermaid / Draw.io / export / score を KatanA から kcf へ移植し、v0.1.2 で Mermaid.js / Draw.io.js の version 固定と更新 recipe を整える。

その次に、v0.1.4 では実際の画面で表示結果を確認できる実表示 E2E（viewer e2e）を追加する。これは ImageMagick score の代替ではなく、score だけでは見落としやすい文字欠け、余白、線の重なり、背景色、実ウィンドウ上の違和感を確認するための補助である。

## What Changes

- `test/e2e/viewer/` に実表示 E2E 専用の検証器（harness）を追加する
- `crates/` 配下の通常 library / CLI には `floem` / `egui` などの画面表示依存を追加しない
- viewer e2e は kcf が出力した SVG / PNG / JPEG / PDF / HTML を読み、実ウィンドウで表示する
- reference 出力と kcf 出力を左右に並べて確認できる画面を提供する
- 最小ケースの起動、入力読み込み、非空表示、スクリーンショット保存を自動 smoke として確認する
- 手動目視確認の観点を `README.md` または E2E report に残す
- `just viewer-e2e-open`、`just viewer-e2e-screenshot`、`just viewer-e2e-smoke` を追加する

## Non-Goals

- ImageMagick score を置き換えない
- `kcf viewer` などの公開 CLI surface を v0.1.4 で固定しない
- `crates/katana-canvas-forge` に KatanA UI state、preview state、workspace state を持ち込まない
- `crates/katana-canvas-forge-cli` に画面表示依存を常用依存として追加しない
- CSV / PDF / Word / Excel / PPTX viewer rendering の本実装は v0.2.0 以降に送る
- egui 版 viewer を初期実装に含めない

## Capabilities

### New Capabilities

- `viewer-e2e-isolation`: 実表示 E2E を `test/e2e/viewer/` に隔離し、core library と CLI の依存を増やさない
- `viewer-e2e-runner`: 画面起動、スクリーンショット保存、smoke 実行を just recipe から呼べる
- `viewer-e2e-review`: 自動 smoke と手動目視確認の境界を明確にする

## Impact

- `test/e2e/viewer/` — 実表示 E2E 専用の検証器、case 定義、README
- `Justfile` — viewer e2e 起動、スクリーンショット、smoke recipe
- `.github/workflows/` — 必要な範囲で viewer e2e smoke を追加
- `openspec/changes/v0-1-4-viewer-e2e-harness/` — 実表示確認の仕様とタスク
