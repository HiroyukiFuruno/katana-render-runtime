# Spec: ZenUML dark mode theme application

## 目的

`RenderThemeMode::Dark` が渡されたとき、ZenUML V8 レンダラーが `@zenuml/core` の
ダークパレットを適用して描画すること。

## 変更対象

| ファイル | 変更内容 |
|---------|---------|
| `crates/katana-canvas-forge/src/markdown/mermaid_renderer/zenuml_v8_runtime.rs` | `_preset` → `preset` に変更し、`is_dark` を preamble に渡す |
| `crates/katana-canvas-forge/src/markdown/mermaid_renderer/js_runtime/render_zenuml_v8.js` | `__zenuml_dark__` を読み取り `renderToSvg` の `options` に渡す |
| `crates/katana-canvas-forge/src/markdown/mermaid_renderer/zenuml_v8_runtime_tests.rs` | ダーク/ライト両モードの unit test を追加 |

## 契約

- `renderToSvg(source, { theme: "dark" })` は `dark_mode: true` のときのみ呼ぶ。
- `dark_mode: false` のときは `renderToSvg(source, {})` または `renderToSvg(source)` を呼ぶ。
- 公開 API (`render()` シグネチャ) は変更しない。

## 完了条件

- `cargo test` が通る。
- `just lint` が通る。
- `zenuml_v8_runtime_tests.rs` にダーク/ライト切り替えのテストが存在する。
- `just mermaid-compare` で `28-zen-uml.md` のスコアが 86.3 以上を維持する。
