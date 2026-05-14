## Context

Mermaid full compare は `28-zen-uml.md` で失敗している。

確認済みの失敗:

- `tests/fixtures/mermaid/en/28-zen-uml.md`
- `tests/fixtures/mermaid/ja/28-zen-uml.md`
- `UnknownDiagramError`

ZenUML は Mermaid 本体だけでは描画されず、ZenUML 対応JSを取り込み、Mermaid runtime に外部 diagram として登録する必要がある。v0.1.2 は未対応扱いを完了条件にしない。コードブロックの構文木（AST）から `zenuml` を判定し、取り込んだJSを使って描画する change である。

## Goals

- `28-zen-uml.md` を通常の Mermaid score 対象として描画する
- ZenUML 対応JSを固定 asset として管理する
- Mermaid render 前に ZenUML 外部 diagram を登録する
- 公式参照生成と Katana runtime の両方で同じ ZenUML 対応方針を使う
- fallback SVG を禁止する

## Implementation Plan

### 1. ZenUML 判定

コードブロックの構文木（AST）を `RenderInput` / `DiagramBlock` に変換する経路で、Mermaid ブロックの source 先頭を正規化して diagram type を判定する。

判定ルール:

- frontmatter がある場合は frontmatter 後の最初の有効行を見る
- 空行と前後空白を無視する
- 先頭 token を小文字化し、`zenuml` と一致したら ZenUML とする

既存の `render_mermaid.js` には `katanaMermaidDiagramType()` があるため、Rust 側に同じ判定を重複実装する場合は専用型か専用関数に分離し、テストで JS 側の期待値と揃える。

### 2. ZenUML 対応JSの取り込み

`@mermaid-js/mermaid-zenuml` 由来の browser 実行可能なJSを vendor asset として固定する。

必須項目:

- version を `scripts/runtime-assets/runtime-asset-common.ts` で管理する
- checksum を記録する
- vendor 配置は `crates/katana-diagram-renderer/vendor/mermaid-zenuml/<version>/mermaid-zenuml.min.js` とする
- `RuntimeAsset::mermaid_zenuml()` を追加し、materialize 対象にする
- `Justfile` に `MERMAID_ZENUML_JS_VERSION` と `MERMAID_ZENUML_JS` を追加する

JS が ESM 形式しかない場合は、project script で IIFE 形式に bundle し、`globalThis.__katanaMermaidZenuml` に export する bridge を同梱する。V8 runtime 内で動的 import に依存してはならない。

### 3. Mermaid runtime への登録

`MermaidRuntimeScripts::source_scripts()` の読み込み順を次の順にする。

1. `mermaid.min.js`
2. `mermaid-zenuml.min.js`
3. `mermaid-zenuml-bridge.js`
4. `render-mermaid.js`

`render_mermaid.js` は render 前に次を実行する。

```js
if (katanaMermaidDiagramType(preparedSource.source) === "zenuml") {
  await mermaidValue.registerExternalDiagrams([globalThis.__katanaMermaidZenuml]);
}
```

`globalThis.__katanaMermaidZenuml` が存在しない場合は即時エラーにし、fallback SVG は生成しない。

### 4. 公式参照生成

`scripts/mermaid/official-renderer.ts` も Katana runtime と同じ ZenUML 対応JSを読み込み、`28-zen-uml.md` の公式参照SVGを生成する。

`diagram-update` / `mermaid-compare-full` は `28-zen-uml.md` を通常 fixture として扱い、暗黙除外や metadata による対象外化をしない。

## Verification

- `28-zen-uml.md` が `UnknownDiagramError` にならない
- `28-zen-uml.md` が score 対象に含まれる
- `registerExternalDiagrams` が render 前に呼ばれる
- ZenUML 対応JSの version / checksum が固定される
- 公式参照生成と Katana runtime の両方で ZenUML を描画できる
- fallback SVG / stub PNG が追加されていない
- `just check`
- `just mermaid-compare-full`
- `npx -y @fission-ai/openspec validate "v0-1-2-mermaid-zenuml-fixture-support" --strict`
