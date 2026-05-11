# zenuml-v8-renderer Specification

## Purpose

ZenUML ダイアグラムを Node.js / Playwright / Chromium に依存せず、
`DiagramV8Runtime`（rusty_v8 in-process）で描画できるようにする。

## Requirements

### Requirement: ZenUML を V8 in-process で描画しなければならない

システムは ZenUML ダイアグラムを `@zenuml/core` の `renderToSvg()` を
V8 in-process で実行して SVG を生成しなければならない（MUST）。
`node` 外部プロセスを起動してはならない（MUST NOT）。
Playwright / Chromium を使用してはならない（MUST NOT）。

#### Scenario: ZenUML fixture を V8 で描画する

- **GIVEN** ZenUML DSL ソース文字列
- **WHEN** `ZenumlV8RenderOps::render()` を呼ぶ
- **THEN** SVG 文字列が返される
- **THEN** 返された SVG に `<svg` タグが含まれる
- **THEN** 返された SVG に `<foreignObject` が含まれない
- **THEN** 外部プロセスが起動されない

### Requirement: `@zenuml/core` asset を固定 version で管理しなければならない

システムは `@zenuml/core` の `zenuml.js` を repository 管理の vendor asset として扱い、
version と checksum を管理しなければならない（MUST）。
外部ネットワーク取得なしで render runtime に読み込めなければならない（MUST）。

#### Scenario: zenuml-core asset を materialize する

- **GIVEN** runtime asset の初期化が実行される
- **WHEN** ZenUML V8 renderer が `zenuml.js` を必要とする
- **THEN** `crates/katana-canvas-forge/vendor/zenuml-core/<version>/zenuml.js` が materialize される
- **THEN** materialize 前後で checksum が一致する

### Requirement: `~~~zenuml` 直接コードブロックを描画しなければならない

システムは `~~~zenuml` を言語タグとするコードブロックを
`~~~mermaid` + `zenuml` キーワードと同等に描画しなければならない（MUST）。

#### Scenario: `~~~zenuml` 直接記法を描画する

- **GIVEN** `` ~~~zenuml `` 言語タグのコードブロック（DSL 本体のみ、`zenuml` キーワード行なし）
- **WHEN** レンダラーがコードブロックを処理する
- **THEN** `ZenumlV8RenderOps::render()` が呼ばれる
- **THEN** SVG 文字列が返される
- **THEN** 返された SVG に `<svg` タグが含まれる

### Requirement: 旧 Node/Playwright レンダラーを削除しなければならない

`zenuml_browser_runtime.rs` および `render_zenuml_browser.mjs` は
リポジトリから削除されなければならない（MUST）。
`node` コマンドへの依存を ZenUML レンダリング目的で持ってはならない（MUST NOT）。
