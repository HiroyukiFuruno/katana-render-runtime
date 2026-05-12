## 設計

### 問題の構造

`ZenumlV8RenderOps::render(source, _preset, _svg_id)` が `_preset` を捨てている。
`DiagramColorPreset.dark_mode` は `RenderThemeMode::Dark` 由来の信頼できる値を持つ。

`@zenuml/core` の `renderToSvg(source, options)` は第2引数で `{ theme: "dark" }` を
受け取り、ダークパレットの CSS 変数を適用する。現行は `renderToSvg(stripped).svg`
として `options` を渡していないため、常にライトモードで描画される。

### 修正方針

1. **Rust 側** (`zenuml_v8_runtime.rs`): `_preset` → `preset` に変更し、`dark_mode`
   を preamble に渡す。`build_preamble(source)` を `build_preamble(source, is_dark)`
   に変更する（または別変数として `__zenuml_dark__` グローバルを追加する）。

2. **JS 側** (`render_zenuml_v8.js`): `__zenuml_dark__` を読み取り、`true` なら
   `renderToSvg(stripped, { theme: "dark" })` を呼ぶ。

### インターフェース

```js
// render_zenuml_v8.js（変更後）
var options = __zenuml_dark__ ? { theme: "dark" } : {};
return zenuml.renderToSvg(stripped, options).svg;
```

```rust
// build_preamble（変更後）
fn build_preamble(source: &str, is_dark: bool) -> String {
    format!(
        "var __zenuml_source__ = {};\nvar __zenuml_dark__ = {};",
        serde_json::to_string(source).unwrap_or_default(),
        is_dark
    )
}
```

### スコープ外

- ZenUML CSS 変数の詳細カスタマイズは対象外。`@zenuml/core` が `"dark"` で定義する
  デフォルトダークパレットをそのまま使う。
- SVG 背景色の注入は `render.rs` 側で既に行われており、変更しない。

### 検証

- `zenuml_v8_runtime_tests.rs` にダーク/ライト両モードの unit test を追加する。
- `just mermaid-compare` で `28-zen-uml.md` のスコアが改善することを確認する
  （ダークテーマ基準: 既存 86.3 以上を維持）。
