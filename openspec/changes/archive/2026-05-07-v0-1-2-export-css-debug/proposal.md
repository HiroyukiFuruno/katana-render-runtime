> Status: 要件変更により破棄済み。`v0.1.2` は Mermaid ZenUML / unsupported fixture handling に再割当する。

## Why

v0.1.2 は、KDV移譲まで維持する既存exportのCSS回帰を止める最小フェーズにする。

KatanA 側では、HTML で指定した `body` の背景色が PDF / PNG / JPEG export に反映されることを確認していた。kdr への移植後、lint 対応などの過程で、`html, body { ... }` や `background: ...` のような指定を native export 側が拾えず、PDF と画像が白背景に戻るデグレードが起きている。

画面上では、デバッグ実行時に HTML / PDF / PNG / JPG が macOS の既定アプリで順番に開く。PDF と画像は `/tmp` に出力する。

## What Changes

- v0.1.2 の目的を export CSS 回帰修正とデバッグ実行に固定する
- native PDF / PNG / JPEG export で、HTML 内の `body` 向け CSS を反映する
- `html, body { background: ...; color: ... }` のようなセレクタ一覧と `background` 省略指定を扱う
- export 4形式をまとめて確認する macOS 専用デバッグコマンドを追加する
- デバッグコマンドは `/tmp` に HTML / PDF / PNG / JPG を出力し、macOS の `open` で既定アプリを開く
- export CSS 回帰テストに PNG / JPEG / PDF を含める
- このexportはKDV同等機能が入るまでの維持対象であり、新規export責務として拡張しない

## Non-Goals

- Windows / Linux のデバッグ起動は v0.1.2 では扱わない
- CSS 全体の描画エンジンを実装しない
- KatanA の画面状態や preview 状態を kdr に持ち込まない
- viewer E2E の本格実装は v0.1.2 に含めない
- 公開用途の安定した viewer CLI として固定しない
- KDV移譲後もKDRがMarkdown exportを所有し続ける前提を作らない

## Capabilities

### New Capabilities

- `export-native-css`: native PDF / PNG / JPEG export が HTML の `body` 向けCSSを反映する
- `export-debug-open`: 4形式を `/tmp` に出力し、macOS の既定アプリで開く

## Impact

- `crates/katana-diagram-renderer/src/markdown/export/` — native export の CSS 解釈を修正
- `crates/katana-diagram-renderer/tests/` — PDF / PNG / JPEG のCSS回帰テストを追加
- `crates/katana-diagram-renderer-cli/` — macOS専用の export デバッグコマンドを追加
- `openspec/changes/archive/2026-05-07-v0-1-2-export-css-debug/` — v0.1.2 の仕様とタスク
