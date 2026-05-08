## Why

v0.1.0 の Mermaid full compare では `28-zen-uml.md` が `UnknownDiagramError` になった。

原因は ZenUML が Mermaid 本体だけでは描画されず、ZenUML 対応JSを取り込み、Mermaid runtime に外部 diagram として登録していないことにある。v0.1.3 では `28-zen-uml.md` を未対応扱いにせず、コードブロックの構文木（AST）から Mermaid / ZenUML を判定し、取り込んだ ZenUML 対応JSで実際に描画する。

## What Changes

- コードブロックの構文木（AST）または既存の `RenderInput` 生成箇所で、Mermaid ブロック内の先頭 diagram type が `zenuml` かを判定する
- ZenUML 対応JSを vendor asset として固定し、version / checksum / download source を管理する
- Mermaid runtime で `mermaid.min.js` の後、`render-mermaid.js` の前に ZenUML 対応JSと bridge script を読み込む
- `zenuml` 判定時は `mermaid.registerExternalDiagrams([zenuml])` 相当の登録を render 前に必ず実行する
- `tests/fixtures/mermaid/en/28-zen-uml.md` と `tests/fixtures/mermaid/ja/28-zen-uml.md` を通常の score 対象として参照SVG / PNGまで更新する
- full compare で `28-zen-uml.md` が `UnknownDiagramError` にならないことを確認する

## Non-Goals

- v0.1.4 の Draw.io / Mermaid score 改善に混ぜない
- Draw.io.js の version 固定は v0.1.1 に送る
- ZenUML 対応に不要な Mermaid.js 本体の更新はしない
- export CSS 回帰修正と macOS debug open はKDV移譲対象として v0.1.6 に送る
- fallback SVG で ZenUML を描けた扱いにしない
- `28-zen-uml.md` を未対応 fixture として score 対象から外さない

## Capabilities

### New Capabilities

- `mermaid-zenuml-rendering`: ZenUML 対応JSを取り込み、Mermaid runtime に登録して描画する

## Impact

- `crates/katana-canvas-forge/vendor/` — ZenUML 対応JSの固定配置
- `scripts/runtime-assets/` — ZenUML 対応JSの version / checksum 管理
- `crates/katana-canvas-forge/src/markdown/runtime_assets.rs` — ZenUML 対応JS asset の materialize
- `crates/katana-canvas-forge/src/markdown/mermaid_renderer/` — ZenUML bridge script 読み込みと render 前登録
- `tests/fixtures/mermaid/` — ZenUML fixture の参照SVG / PNG更新
- `scripts/mermaid/` — 公式参照生成側でも ZenUML 対応JSを読み込む
- `openspec/changes/v0-1-3-mermaid-zenuml-fixture-support/` — 本 change の仕様とタスク
