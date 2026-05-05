## Why

v0.1.0 は初回移植版として release するが、KatanA 側へまだ取り込まないため、既存品質は劣化しない。

一方で、移植時の full compare では Draw.io official diagrams の `i18n` と `link` が 99 点に届かず、代表ケースにも既知下限が残っている。これは v0.1.0 の公開を止めるのではなく、v0.1.1 の小規模 patch として Jules CLI に渡せる作業単位へ分離する。

## What Changes

- Draw.io official / representative の score 未達ケースを調査し、描画差分の原因を分類する
- 修正できる描画差分は renderer / resource resolver / postprocess で改善する
- 既知下限を下げるためではなく、現在値から score floor を上げるために baseline を更新する
- Mermaid の supported fixture は score 回帰がないことを確認する
- score report と contact sheet を PR review で確認できる成果物として残す

## Non-Goals

- v0.1.0 release を延期しない
- Mermaid ZenUML fixture の対応は v0.1.3 に送る
- Mermaid.js / Draw.io.js の version 固定と取り込み更新は v0.1.2 に送る
- viewer e2e harness は v0.1.4 に送る
- KatanA 側の consume 作業はこの change に含めない

## Capabilities

### New Capabilities

- `reference-score-improvement`: Draw.io / Mermaid の既知 score 未達を改善し、baseline を上げる
- `reference-score-reporting`: 改善前後の score と contact sheet を review できる形で残す

## Impact

- `crates/katana-canvas-forge/src/markdown/drawio_renderer/` — Draw.io renderer / resource resolver / SVG postprocess
- `scripts/drawio/` — score 比較、report、baseline policy
- `scripts/mermaid/` — supported fixture の score 回帰確認
- `tests/fixtures/drawio/representative/score-baseline.json` — 既知下限の改善
- `tests/fixtures/drawio/` — score 改善対象 fixture と reference
- `openspec/changes/v0-1-1-score-improvement/` — 本 change の仕様とタスク
