## Why

`RenderThemeMode::Dark` を渡しても ZenUML 図の内部カラー（ライフライン背景、
Alt/Par ブロック、テキスト色など）がライトモードのまま描画される。

KatanA 側 (`kdr_theme_adapter.rs`) は `RenderThemeSnapshot.mode = Dark` を正しく
設定している。問題は kdr の ZenUML V8 レンダラー側にある。

`@zenuml/core` の `renderToSvg()` はオプションで `theme` を受け取り、
`"dark"` を渡すことで内部 CSS 変数がダークパレットに切り替わる。
現行の `render_zenuml_v8.js` は `__zenuml_theme__` を渡してはいるが、
`renderToSvg()` の `theme` オプションに正しく接続されていない可能性がある。

## What Changes

`render_zenuml_v8.js` において `__zenuml_theme__` を `renderToSvg()` の `theme`
オプションへ正しくマッピングし、ダークモード時に `@zenuml/core` がダークパレットの
CSS 変数を適用するようにする。

| 変更前 | 変更後 |
|--------|--------|
| `renderToSvg(source)` — theme 未指定 | `renderToSvg(source, { theme: "dark" })` — `RenderThemeMode::Dark` 時 |

合わせて、レンダリングスコア（`28-zen-uml` fixture）がダークテーマで改善することを
確認する。

## Deliverables

- `crates/katana-diagram-renderer/src/markdown/mermaid_renderer/js_runtime/render_zenuml_v8.js` — theme オプション接続
- `crates/katana-diagram-renderer/src/markdown/mermaid_renderer/zenuml_v8_runtime.rs` — theme 引数の pass-through 確認
- `openspec/changes/v0-1-6-zenuml-dark-theme/` — 本変更記録
