## Why

v0.1.3 は export の CSS 回帰を止める最小フェーズにする。

KatanA 側では、HTML で指定した `body` の背景色が PDF / PNG / JPEG export に反映されることを確認していた。kcf への移植後、lint 対応などの過程で、`html, body { ... }` や `background: ...` のような指定を native export 側が拾えず、PDF と画像が白背景に戻るデグレードが起きている。

画面上では、デバッグ実行時に HTML / PDF / PNG / JPG が macOS の既定アプリで順番に開く。PDF と画像は `/tmp` に出力する。

## What Changes

- v0.1.3 の目的を export CSS 回帰修正とデバッグ実行に差し替える
- native PDF / PNG / JPEG export で、HTML 内の `body` 向け CSS を反映する
- `html, body { background: ...; color: ... }` のようなセレクタ一覧と `background` 省略指定を扱う
- export 4形式をまとめて確認する macOS 専用デバッグコマンドを追加する
- デバッグコマンドは `/tmp` に HTML / PDF / PNG / JPG を出力し、macOS の `open` で既定アプリを開く
- export CSS 回帰テストに PNG / JPEG / PDF を含める

## Non-Goals

- Windows / Linux のデバッグ起動は v0.1.3 では扱わない
- CSS 全体の描画エンジンを実装しない
- KatanA の画面状態や preview 状態を kcf に持ち込まない
- viewer E2E の本格実装は v0.1.3 に含めない
- 公開用途の安定した viewer CLI として固定しない

## Capabilities

### New Capabilities

- `export-native-css`: native PDF / PNG / JPEG export が HTML の `body` 向けCSSを反映する
- `export-debug-open`: 4形式を `/tmp` に出力し、macOS の既定アプリで開く

## Impact

- `crates/katana-canvas-forge/src/markdown/export/` — native export の CSS 解釈を修正
- `crates/katana-canvas-forge/tests/` — PDF / PNG / JPEG のCSS回帰テストを追加
- `crates/katana-canvas-forge-cli/` — macOS専用の export デバッグコマンドを追加
- `openspec/changes/v0-1-3-export-css-debug/` — v0.1.3 の仕様とタスク
