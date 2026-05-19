## Context

Issue #7 では、Draw.io source の `value="First line&#10;Second line"` が KDR 出力の SVG fallback text で `First line Second line` に潰れることが確認されている。
KatanA 側は `foreignObject` を落として fallback `<text>` を使う経路があるため、この fallback が1行化すると画面でも改行なしに見える。

この release には、既に実施済みの Draw.io.js 30.0.2 更新も含める。
更新自体は再実行せず、既存差分を release commit / push の対象として扱う。

## Decision

- `katanaNormalizedDrawioHtmlTextFallbackNode()` の `tspans.length === 0` 分岐で、複数行を空白結合しない
- fallback `<text>` に既存 `<tspan>` が無い場合は、既存の `katanaDrawioLineBreakTspan()` で行ごとの `<tspan>` を生成する
- 既存 `<tspan>` がある場合の merge 動作は維持し、今回の修正範囲を plain text fallback の欠落分に限定する
- `drawio-runtime.min.js` は source から `just runtime-bundle-build` で再生成し、手編集しない
- Draw.io.js 30.0.2 更新済みの vendor asset、checksum、reference snapshot は release に含める

## Scope

対象:

- Draw.io plain text label の fallback SVG 改行保持
- Draw.io.js 30.0.2 固定 version 更新の release 取り込み
- `v0.1.3` crate version bump

対象外:

- KatanA 側の SVG rasterize（ラスタライズ: SVG を画像に変換する処理）変更
- Draw.io の label layout 全般の再設計
- score threshold（しきい値）の引き下げ
- ESM runtime bundles active change の実装

## Verification

- 先に回帰 test を追加し、修正前に失敗することを確認する
- 修正後に focused Rust test を通す
- `just runtime-bundle-check` で source と generated bundle の同期を確認する
- `just check`
- `just VERSION=v0.1.3 release-check`
- `git diff --check`
- OpenSpec strict validate
