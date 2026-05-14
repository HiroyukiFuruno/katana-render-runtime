## Context

> Status: 要件変更により破棄済み。`v0.1.2` は Mermaid ZenUML / unsupported fixture handling に再割当する。

v0.1.2 は、v0.1.0 transfer と v0.1.1 runtime asset version pinning の後続である。

このフェーズでは、KDV移譲まで維持する既存exportで生成される PDF / PNG / JPEG に HTML の CSS が当たらない回帰を止める。特に `html, body { background: ...; color: ... }` のような selector list と、`background-color` ではなく `background` を使う指定を対象にする。

画面上では、デバッグコマンドを実行すると HTML / PDF / PNG / JPG がそれぞれ macOS の既定アプリで開く。確認者は同じ入力HTMLから出た4形式を並べて、背景色、文字色、余白の崩れを見比べられる。

## Goals

- PDF / PNG / JPEG export が `body` 向け CSS を反映する
- `html, body { ... }` のような selector list を扱える
- `background: #xxxxxx` のような background 省略指定を扱える
- JPEG も回帰テスト対象に含める
- `kdr export-debug --input <html>` で4形式をまとめて出力できる
- デバッグ出力は `/tmp` に置き、macOS の `open` で既定アプリを開く

## Non-Goals

- CSS cascade 全体を実装しない
- class / id / media query / external stylesheet の完全対応は扱わない
- Windows / Linux の `start` / `xdg-open` 対応は扱わない
- viewer E2E の左右比較画面はこの change に含めない
- KDV実装後もKDRがMarkdown exportを所有し続ける前提は作らない

## CSS Parsing Boundary

native export は HTML を完全なブラウザとして描画していない。v0.1.2 では、既存の軽量CSS解釈を広げ、`<style>...</style>` 内から `body` を対象にした規則を探す。

対象にする指定:

- `body { background-color: #1e1e1e; color: #e0e0e0; }`
- `html, body { background: #1e1e1e; color: #e0e0e0; }`
- `body.dark { ... }` や `body[data-theme="dark"] { ... }`

対象外:

- `@media` 内の条件分岐
- external CSS file
- CSS custom property の解決
- 詳細な specificity 判定

## Debug Command Design

デバッグコマンドは通常の `export` と分ける。

- entrypoint: `kdr export-debug --input <html>`
- output directory: `/tmp`
- output files: `kdr-export-debug-<pid>.html`, `.pdf`, `.png`, `.jpg`
- open order: HTML, PDF, PNG, JPG
- opener: macOS の `open`

このコマンドは開発確認用であり、通常の export API の代替ではない。macOS 以外の分岐は入れない。

このdebug commandもKDV移譲までの既存export保守用である。

## Verification Boundary

自動検証で扱うもの:

- CSS parser が selector list と `background` 省略指定を拾う
- PNG / JPEG / PDF のサンプルピクセルが HTML の暗色背景になる
- CLI が `export-debug` を parse できる
- デバッグコマンドが4形式を `/tmp` に出力し、4回 opener を呼ぶ

手動確認で扱うもの:

- macOS の既定アプリで開いたときの見え方
- 背景色、文字色、余白の違和感
- 画像とPDFの体感差

## Version Dependencies

- `v0.1.0`: renderer / exporter / score の transfer が前提
- `v0.1.1`: runtime asset version pinning が前提
- `v0.1.2`: export CSS 回帰修正と debug open
- `v0.1.3`: Mermaid ZenUML / unsupported fixture handling
- `v0.1.4`: reference score improvement

## Risks

- 軽量CSS解釈の範囲を広げすぎると、ブラウザ互換の責務を背負ってしまう
- `/tmp` の固定出力は同一プロセスで上書きされる
- `open` は手元の既定アプリ設定に依存するため、自動テストでは fake opener で確認する
