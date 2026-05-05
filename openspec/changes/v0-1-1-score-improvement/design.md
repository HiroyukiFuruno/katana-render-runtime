## Context

v0.1.0 の release-check は通過しているが、local full compare では Draw.io official diagrams に 99 点未満が残っている。

確認済みの代表例:

- `i18n`: 94.32
- `link`: 97.38

v0.1.0 はまだ KatanA に取り込まないため release する。ただし、score が低いまま KatanA 側の検証を削ぐと品質低下を見逃すため、v0.1.1 で score 改善を先に実施する。

## Goals

- Draw.io の既知 score 未達 case を原因別に分類する
- 修正可能な差分は renderer / resource resolver / SVG postprocess 側で直す
- baseline は現在値を追認するためではなく、改善後の下限として更新する
- score report と contact sheet を Jules / PR review が確認できる形で残す

## Non-Goals

- Mermaid ZenUML の描画対応はしない
- unsupported fixture を暗黙 skip しない
- score 閾値を下げて合格扱いにしない
- fallback SVG や stub PNG で比較を通さない

## Score Policy

v0.1.1 は「最低点を上げる patch」である。

まず `drawio-compare-ci` と full compare の early-fail 箇所を確認し、score 未達の原因を次に分類する。

- runtime / resource 解決漏れ
- SVG postprocess の差分
- browser rasterize の差分
- 公式 reference 側の特殊ケース
- 現時点では修正しない既知差分

修正しない既知差分は、理由と現在 score を report に残す。ただし、baseline は現在値より低くしない。

## Verification

- `just drawio-compare-ci`
- `just drawio-compare-full`
- supported Mermaid fixture の compare
- `just check`
- `just VERSION=v0.1.1 release-check`
- `npx -y @fission-ai/openspec validate "v0-1-1-score-improvement" --strict`

Mermaid ZenUML 由来の failure は v0.1.3 の対象として扱い、v0.1.1 の score 改善完了条件には含めない。
