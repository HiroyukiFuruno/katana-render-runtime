## Why

ZenUML の現行レンダラー（`zenuml_browser_runtime.rs`）は Node.js 外部プロセスを起動し、
その中で Playwright（Chromium）を使って DOM レンダリングを行う。

この設計には 2 つの根本的な問題がある。

**Layer 1 — $PATH 問題**
macOS packaged .app はシェルの `$PATH` を継承しないため、Homebrew / nvm / asdf で
インストールした `node` を見つけられず、ZenUML が常にクラッシュする。

**Layer 2 — ユーザー環境依存**
Playwright と Chromium バイナリがユーザー環境に別途インストールされていることを前提とする。
エンドユーザーが `playwright install chromium` を実行していることを期待するのは、
配布アプリとして成立しない。

## What Changes

現行の `ZenumlBrowserRenderOps`（Node + Playwright）を廃止し、
`@zenuml/core` が公開する `renderToSvg()` 関数を kdr の `DiagramV8Runtime` で
インプロセス実行する新しいレンダラーに置き換える。

**Spike 検証済みの根拠（2026-05-12, `spike/zenuml-optimize`）：**

- `@zenuml/core` の `zenuml.js` は完全に self-contained なバンドル（`require()` なし）
- `renderToSvg(code, options)` は DOM なしで呼び出し可能
- `<canvas>` がない場合は自動的に character-estimate fallback（`Math.ceil(len * 16 * 0.6)`）
  に切り替わり、正常に SVG を生成する
- 生成された SVG は `<foreignObject>` を含まず、Playwright 版と同等の構造

## Scope 追加: `~~~zenuml` 直接コードブロック対応

現状 kdr は `~~~mermaid` + `zenuml` キーワードのみを ZenUML として認識する。
`~~~zenuml` を言語タグとして直接使う記法（zenuml.com standalone と同じ書き方）にも対応する。

| コードブロック | 動作 |
|--------------|------|
| `` ~~~mermaid `` + `zenuml` キーワード | 先頭行を strip して `renderToSvg()` に渡す（既存） |
| `` ~~~zenuml `` 直接 | そのまま `renderToSvg()` に渡す（新規） |

## Deliverables

- `crates/katana-diagram-renderer/src/markdown/mermaid_renderer/zenuml_v8_runtime.rs` — 新レンダラー
- `crates/katana-diagram-renderer/src/markdown/mermaid_renderer/js_runtime/render_zenuml_v8.js` — V8 用 JS ブリッジ
- `crates/katana-diagram-renderer/vendor/zenuml-core/<version>/zenuml.js` — `@zenuml/core` vendor asset
- `~~~zenuml` 言語タグの markdown パーサ対応
- 旧 `zenuml_browser_runtime.rs` / `render_zenuml_browser.mjs` の削除
- `openspec/changes/v0-1-5-zenuml-v8-renderer/` — 本変更記録
