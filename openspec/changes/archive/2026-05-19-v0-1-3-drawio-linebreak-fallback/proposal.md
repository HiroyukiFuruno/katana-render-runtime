## Why

Draw.io の `&#10;` 改行ラベルは、KDR が生成する SVG fallback text で1行に潰れ、KatanA の画面上でも `First line Second line` のように表示される。
`v0.1.3` ではこの表示不具合を直し、同じ release に既に更新済みの Draw.io.js 30.0.2 取り込みも含める。

## What Changes

- Draw.io の plain text label（素の文字ラベル）に含まれる `&#10;` / 改行を、SVG fallback 側でも複数行として保持する
- fallback `<text>` に `<tspan>` が無い場合でも、行ごとの `<tspan>` を生成する
- 生成済み `drawio-runtime.min.js` と checksum を source から再生成する
- 既に実施済みの Draw.io.js 30.0.2 更新、checksum、reference snapshot 更新を `v0.1.3` に含める
- workspace crate version を `0.1.3` に上げる

## Capabilities

### New Capabilities

- `drawio-text-fallback-rendering`: Draw.io の HTML / plain text label を、`foreignObject` が使えない描画経路でも崩さず読める SVG fallback として保持する

### Modified Capabilities

- `runtime-asset-versioning`: `v0.1.3` で Draw.io.js の固定 version を 30.0.2 に更新し、checksum と reference snapshot を同期済み artifact として扱う

## Impact

- `crates/katana-diagram-renderer/src/markdown/drawio_renderer/js_runtime/drawio_svg_html_text_labels.js`
- `crates/katana-diagram-renderer/src/markdown/diagram_runtime/generated/drawio-runtime.min.js`
- `crates/katana-diagram-renderer/src/markdown/diagram_runtime/generated/runtime-bundles.sha256`
- `crates/katana-diagram-renderer/src/markdown/drawio_renderer/js_runtime_plain_text_label_tests.rs`
- `crates/katana-diagram-renderer/vendor/drawio/30.0.2/`
- `tests/fixtures/drawio/**/official/*.{svg,png}`
- `Cargo.toml`, `Cargo.lock`, `crates/katana-diagram-renderer-cli/Cargo.toml`
