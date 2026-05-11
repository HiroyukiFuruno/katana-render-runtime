# Tasks: katana-canvas-forge v0.1.4 ZenUML foreignObject fix

## Branch Rule

- **標準ブランチ**: `release/v0.1.4`
- **作業ブランチ**: `feature/v0.1.4-task-x`

---

## 1. render_zenuml_browser.mjs の出力変更

### 目的

`render_zenuml_browser.mjs` を「SVG 文字列を stdout に出力する」から「PNG を base64 埋め込みした SVG ラッパーを stdout に出力する」に変更する。

### タスク

- [x] 1.1 `page.locator('#diagram')` の bounding box を `boundingBox()` で取得する処理を `renderZenuml` 関数の末尾に追加する
- [x] 1.2 bounding box の width または height が 0 以下の場合は `throw new Error("ZenUML bounding box is empty")` を投げる
- [x] 1.3 `page.screenshot({ clip: bbox, type: 'png' })` で PNG bytes を取得する
- [x] 1.4 PNG bytes を `Buffer.from(png).toString('base64')` で base64 エンコードする
- [x] 1.5 `<svg xmlns="..." xmlns:xlink="..." width="{W}" height="{H}" viewBox="0 0 {W} {H}"><image xlink:href="data:image/png;base64,{BASE64}" width="{W}" height="{H}"/></svg>` 形式の SVG 文字列を生成する
- [x] 1.6 `mermaid.render()` の SVG を `console.log` する代わりに、1.5 で生成した SVG ラッパーを `console.log` する
- [x] 1.7 `node --input-type=module --eval` で `render_zenuml_browser.mjs` を直接起動し、ZenUML ソースを stdin に渡して出力 SVG に `<foreignObject` が含まれないことを手動確認する

---

## 2. テストの期待値更新

### 目的

`js_runtime_tests.rs` の ZenUML テストを「foreignObject を含まない SVG」の期待値に更新する。

### タスク

- [x] 2.1 `js_runtime_tests.rs` の `zenuml_source_uses_browser_renderer_from_mermaid_surface` テストの期待値から `"<foreignObject"` を削除する
- [x] 2.2 同テストに `!svg.contains("<foreignObject")` のアサーションを追加する
- [x] 2.3 同テストに `svg.contains("<image")` と `svg.contains("data:image/png;base64,")` のアサーションを追加する
- [x] 2.4 `cargo test zenuml_source_uses_browser_renderer` を実行してテストが通ることを確認する

---

## 3. ネイティブ画像化 regression テスト追加

### 目的

ZenUML 出力を KatanA 相当のネイティブ画像化経路（foreignObject 削除後ラスタライズ）に通して非空・非白になることを確認する。

### タスク

- [x] 3.1 `svg_rasterize_tests.rs` に `zenuml_output_rasterizes_to_non_blank_image` テストを追加する
- [x] 3.2 テスト内で `MermaidJsRuntimeOps::render("zenuml\ntitle Test\nA.method()", ...)` を呼び出す
- [x] 3.3 返された SVG を `SvgRasterizeOps::remove_foreign_objects` 相当の処理に通す
- [x] 3.4 処理後の SVG を `SvgRasterizeOps::rasterize_svg` でラスタライズし、結果が空でないことを確認する
- [x] 3.5 ラスタライズ結果の PNG バイト列を `image` crate（または `png` crate）でデコードし、全ピクセル白でないことを確認する
- [x] 3.6 `cargo test zenuml_output_rasterizes_to_non_blank` を実行してテストが通ることを確認する

---

## 4. Quality Gate

- [x] 4.1 `cargo test` を実行して全テストが通ることを確認する
- [x] 4.2 `just check` を実行する
- [x] 4.3 `just VERSION=v0.1.4 release-check` を実行する
- [x] 4.4 `git diff --check` を実行する

---

## 5. Final Verification

- [x] 5.1 `openspec status --change v0-1-4-zenuml-foreignobject-fix` で全 artifact が done になっていることを確認する
- [ ] 5.2 PR 作成後に `@codex review` を依頼する
- [x] 5.3 `npx -y @fission-ai/openspec validate "v0-1-4-zenuml-foreignobject-fix" --strict` を実行する
- [ ] 5.4 指摘があれば対応し、再度 `@codex review` を依頼する
