## Context

### 現行アーキテクチャ

```
kcf (Rust)
  └─ ZenumlBrowserRenderOps::render()
       └─ ProcessService::create_command("node")   ← $PATH 依存
            └─ render_zenuml_browser.mjs
                 └─ playwright / chromium           ← ユーザー環境依存
```

### 新アーキテクチャ

```
kcf (Rust)
  └─ ZenumlV8RenderOps::render()
       └─ DiagramV8Runtime::render(scripts)         ← in-process
            ├─ zenuml.js  (@zenuml/core vendor)
            └─ render_zenuml_v8.js  (ブリッジ)
```

## Design Decisions

### D1: `@zenuml/core` の `zenuml.js` を vendor asset として管理する

`mermaid-zenuml.min.js` は mermaid plugin API（`loader` / `renderer` / `parser`）経由で
React DOM に描画するため、DOM なしでは SVG を返せない。

一方 `@zenuml/core` の `renderToSvg()` は SVG 文字列を直接返す API であり、
DOM / canvas がなくても character-estimate fallback で動作する（Spike 検証済み）。

`zenuml.js`（CJS バンドル）を `crates/katana-canvas-forge/vendor/zenuml-core/<version>/`
に配置し、`mermaid-zenuml.min.js` と同様に runtime asset として materialize する。

### D2: `mermaid-zenuml.min.js` vendor asset は保持する

mermaid の ZenUML 検出（`detector`）と type 判定は `mermaid-zenuml.min.js` の `id` /
`detector` に依存している。V8 レンダラー側ではこれを使わないが、
`render_mermaid.js` の diagram type 判定フローを変更しないため残す。

ZenUML と判定されたソースだけを `ZenumlV8RenderOps` にルーティングする。

### D3: `render_zenuml_v8.js` は薄いブリッジとする

```js
// render_zenuml_v8.js
const result = renderToSvg(__zenuml_source__, { theme: __zenuml_theme__ });
__zenuml_result__ = result.svg;
```

グローバル変数 `__zenuml_source__`, `__zenuml_theme__` を Rust 側から注入し、
`__zenuml_result__` を読み取る。`DiagramV8Runtime` の既存パターンに合わせる。

### D4: canvas polyfill は提供しない

Spike で確認した通り、canvas がない場合は ZenUML が自動的に
character-estimate fallback（`Math.ceil(len * 16 * 0.6)`）を使用する。
精度は canvas 計測より若干落ちるが、参加者名が短い通常のダイアグラムでは許容範囲内。

canvas polyfill（`node-canvas` 相当）は V8 in-process では提供困難であり、
現時点では不要と判断する。

### D5: 旧 `ZenumlBrowserRenderOps` は完全削除する

Node / Playwright パスを残すと将来の混乱を招く。
`zenuml_browser_runtime.rs` および `render_zenuml_browser.mjs` を削除する。

### D6: `~~~zenuml` 直接コードブロックに対応する

markdown パーサがコードブロックの言語タグを見てレンダラーを選択する。
現状は `mermaid` タグのみを `MermaidRenderer` に渡しているが、
`zenuml` タグも `ZenumlV8RenderOps` に直接ルーティングする。

| 入力 | 処理 |
|------|------|
| `` ~~~mermaid `` + 先頭行 `zenuml` | `MermaidDiagramType::Zenuml` 判定 → 先頭行 strip → `ZenumlV8RenderOps` |
| `` ~~~zenuml `` | 言語タグで直接 → strip 不要 → `ZenumlV8RenderOps` |

いずれも同じ `ZenumlV8RenderOps::render()` を呼ぶ。
strip の要否は呼び出し元で制御し、`ZenumlV8RenderOps` 自体は DSL 本体のみを受け取る。

## Version Bump

`Cargo.toml` の `version` を `0.1.4` → `0.1.5` に上げる。
