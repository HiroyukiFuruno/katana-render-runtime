## Context

v0.1.0 の目的は、KatanA 既存の interface と rendering/export runtime を kcf に忠実移植すること。新規に簡略実装を作る作業ではない。

PR #1 は close 済み。実装差分は土台にしない。レビュー履歴は、避けるべき方針と検証漏れの参照資料として使う。

## Goals

- KatanA の Mermaid 実装を kcf へ移植する
- KatanA の Draw.io 実装と resource 一式を kcf へ移植する
- KatanA の HTML / PDF / PNG / JPEG export 実装を kcf へ移植する
- KatanA の Mermaid / Draw.io reference 生成と ImageMagick 採点評価を kcf へ移植する
- KatanA 側で定義済みの `Renderer` / `Exporter` interface と DTO を完全踏襲する
- kcf は KatanA UI state に依存しない library と CLI を提供する

## Non-Goals

- PR #1 の簡略 Mermaid 実装を育てない
- KatanA 側で定義済みの interface を kcf 独自都合で縮小、改名、簡略化しない
- HTML のみ export を v0.1.0 完了扱いにしない
- SVG 文字列比較だけの score を採点評価として採用しない
- Mermaid.js / Draw.io.js の取り込み version 固定、最新版確認、取り込み just recipe は v0.1.1 に送る
- CSV / PDF / Word / Excel / PPTX viewer rendering は v0.1.0 に含めない
- Native backend 化や外部プロセス依存ゼロ化は v0.1.0 の目的にしない

## Source Of Truth

移植元は KatanA 側の既存実装を正本とする。

- Mermaid runtime: `crates/katana-core/src/markdown/mermaid_renderer/`
- Draw.io runtime: `crates/katana-core/src/markdown/drawio_renderer/`
- export runtime: `crates/katana-core/src/markdown/export/`
- Mermaid scoring: `scripts/mermaid/`
- Draw.io scoring: `scripts/drawio/`
- Mermaid fixtures: `assets/fixtures/mermaid_parts/`
- Draw.io fixtures: `assets/fixtures/drawio/`
- export tests: `crates/katana-core/tests/export_regression.rs`
- visual export tests: `crates/katana-core/tests/native_export_visual.rs`
- Draw.io tests: `crates/katana-core/tests/markdown_drawio*.rs`
- Mermaid tests: `crates/katana-core/tests/markdown_mermaid.rs` と `mermaid_js_runtime_*.rs`

## Boundary Design

kcf は KatanA を知らない library として設計する。

- `Renderer` / `Exporter` は KatanA 側で定義済みの interface を正本として移植する
- `Renderer` は Mermaid / Draw.io の SVG とメタデータを返す
- `Exporter` は HTML / PDF / PNG / JPEG の出力 path と format を返す
- CLI は library の薄い利用者に留める
- KatanA UI state、preview state、workspace state は DTO に持ち込まない
- v0.1.0 では既存 runtime asset を動作可能な形で移す
- vendor bundle、checksum、version pinning、resource manifest の整理は v0.1.1 で kcf 管理に固定する
- KatanA 側は git tag pinned dependency と adapter のみを持つ

## Transfer Approach

移植は責務単位で行う。

1. KatanA 側の現行実装とテストを棚卸しする
2. KatanA 側で定義済みの interface と DTO を棚卸しし、kcf の公開 API として踏襲する
3. Mermaid backend を移植し、既存 fixture と reference score を維持する
4. Draw.io backend と resource resolver を移植し、既存 fixture と reference score を維持する
5. HTML / PDF / PNG / JPEG export を移植し、既存回帰テストを維持する
6. CLI、just、CI に reference-update / compare / bench / export を接続する
7. KatanA 側で kcf を consumer として組み込み、hybrid 状態を残さない

## PR #1 Reference Policy

PR #1 は実装資産として使わない。

- close 済み PR として履歴を残す
- branch は必要に応じて参照する
- 指摘済みの失敗例を v0.1.0 の検証観点へ反映する
- PR #1 の差分を merge / cherry-pick しない

## Version Roadmap

transfer が完了した後、次の versioned change として扱う。

- `v0.2.0`: CSV viewer rendering
- `v0.3.0`: PDF viewer rendering
- `v0.4.0`: Office viewer rendering。対象は Word / Excel / PPTX に限定する
- `v0.4.x`: バグ取りと score 向上
- `v0.5.0`: CLI 公開

viewer rendering は export とは別責務。export は「文書を外部ファイルへ書き出す」処理であり、viewer rendering は「既存ファイルを画面表示向けに描画する」処理である。
