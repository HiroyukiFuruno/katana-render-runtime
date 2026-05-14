## Context

ZenUML は Mermaid の `registerExternalDiagrams` を通じて描画されるが、描画結果が `<foreignObject>` + HTML/CSS の組み合わせになる。これは ZenUML の仕様上の特性であり、JS runtime で SVG ネイティブ要素へ変換することは現実的でない。

KatanA の preview pipeline は kdr が返す SVG を `resvg`/`usvg` でラスタライズする前に `foreignObject` を削除する（`svg_rasterize.rs:70-84`）。このため ZenUML の描画内容が完全に消えて白表示になる。

現在の ZenUML render パス（`render_zenuml_browser.mjs`）はすでに Playwright を使った Chromium ヘッドレスブラウザ経由で動いている。`page.screenshot()` を使えば描画済みの DOM を PNG として取得できる。

## Goals / Non-Goals

**Goals:**
- ZenUML 出力から `<foreignObject>` を除去し、resvg/usvg で空白にならない SVG を返す
- 既存の `MermaidRenderer` が返す `DiagramResult::Ok(String)` 型契約を維持する（SVG 文字列を返す）
- ZenUML テストを「ネイティブ画像化経路で非空・非白」の期待値に更新する

**Non-Goals:**
- `foreignObject` を SVG ネイティブ要素に変換しない（ZenUML 仕様上困難）
- KatanA 側の `foreignObject` 削除ロジックを変更しない
- ZenUML 専用の WebView / 二重 Playwright 経路を KatanA 側に追加しない
- Mermaid 通常ダイアグラムの render パスを変更しない

## Decisions

### 決定: Playwright screenshot を base64 PNG として `<image>` 要素に埋め込んだ SVG ラッパーを返す

`render_zenuml_browser.mjs` において、現在の `mermaid.render()` → SVG 文字列取得の後に以下を追加する。

1. `page.locator('#diagram')` の bounding box を取得する
2. `page.screenshot({ clip: bbox, type: 'png' })` で PNG bytes を取得する
3. PNG を base64 エンコードし、`<image xlink:href="data:image/png;base64,..." width=... height=... />` を内包する SVG ラッパーを生成して stdout に出力する

返す SVG の形式:
```xml
<svg xmlns="http://www.w3.org/2000/svg"
     xmlns:xlink="http://www.w3.org/1999/xlink"
     width="{W}" height="{H}" viewBox="0 0 {W} {H}">
  <image xlink:href="data:image/png;base64,{BASE64}" width="{W}" height="{H}"/>
</svg>
```

この形式は `<foreignObject>` を含まず、resvg/usvg が直接処理できる。既存の `DiagramResult::Ok(String)` 型は変更しない。

**代替案: PNG バイト列を新しい output variant として返す**
- `DiagramResult` に `Png(Vec<u8>)` variant を追加する案は、KatanA 側 consumer が ZenUML だけ別経路で処理する必要が生じ、issue #8 の非方針（consumer 側の実行環境を分けない）に反する。

**代替案: foreignObject 内 HTML を SVG text/path に変換する**
- ZenUML の HTML 構造は CSS layout に依存しており、レイアウト計算なしに SVG text 要素へ変換することは困難。品質劣化が大きく採用しない。

### 決定: Playwright 上で ZenUML SVG が完全に描画された後にスクリーンショットを取る

`mermaid.render()` の完了後、`page.evaluate()` 内で ZenUML SVG の `#diagram` への DOM 挿入を確認してからスクリーンショットを取る。`waitForSelector` または `evaluate` 内の同期的な DOM 参照で実現する。

### 決定: テストの期待値を「`<image` を含む、かつ `<foreignObject` を含まない」に更新する

`js_runtime_tests.rs` の `zenuml_source_uses_browser_renderer_from_mermaid_surface` テストは、現在 `<foreignObject` の存在を期待している。この期待値を削除し、`<image` / `data:image/png;base64` が含まれ `<foreignObject` が含まれないことに変更する。

合わせて `svg_rasterize_tests.rs` に ZenUML 出力をネイティブ画像化する regression テストを追加する。

## Risks / Trade-offs

- [PNG 品質] resvg が base64 PNG を `<image>` 経由でデコードする際、サイズが大きいと遅くなる可能性がある → 現在の viewport は `1520×845` で固定されており許容範囲内。必要なら clip bbox に絞る。
- [bounding box が 0 のケース] `#diagram` の bbox が取れない場合、空 PNG になる → `width ≤ 0` または `height ≤ 0` の場合はエラーとして返す。
- [Playwright バージョン依存] `page.screenshot({ clip })` は Playwright 安定版で動作する。現在の `render_zenuml_browser.mjs` はすでに Playwright に依存しているため追加依存は発生しない。
