## Why

v0.1.0 は初回移植版として release するが、KatanA 側へまだ取り込まないため、既存品質は劣化しない。

一方で、移植時の full compare では Draw.io official diagrams の `i18n` と `link` が 99 点に届かず、代表ケースにも既知下限が残っている。これは v0.1.0 の公開を止めるのではなく、v0.1.x の最終フェーズで扱う。

この範囲は旧 v0.1.1 / v0.1.2 として Jules 側へ渡していたが、粒度が大きく進捗が停滞している。Mermaid ZenUML / unsupported fixture handling を v0.1.3 として閉じ、score 改善（score improvement）を v0.1.4 の最終フェーズへ送る。旧export CSS debugはKDV移譲対象として v0.1.6 に送る。

v0.1.4 の対象は CI 代表ケースだけではなく、full compare が見る全 supported pattern である。完了基準は Draw.io / Mermaid ともに 99 点以上とし、CI compare は高速な代表確認、full compare は完了判定として扱う。

## What Changes

- Draw.io / Mermaid の full compare 対象にある全 supported pattern の score 未達ケースを調査し、描画差分の原因を分類する
- 修正できる描画差分は renderer / resource resolver / postprocess で改善する
- 既知下限を下げるためではなく、99 点以上へ上げるために baseline / accepted score floor を更新する
- Mermaid の supported fixture も full compare で 99 点以上を満たす
- v0.1.3 で固定した unsupported fixture metadata を前提に、unsupported fixture を暗黙 skip しない
- score report と contact sheet を PR review で確認できる成果物として残す

## Non-Goals

- v0.1.0 release を延期しない
- Mermaid ZenUML fixture の対応は v0.1.3 に送る
- Mermaid.js / Draw.io.js の version 固定と取り込み更新は v0.1.1 に送る
- export CSS 回帰修正と macOS debug open はKDV移譲対象として v0.1.6 に送る
- KatanA 側への取り込み（consume）作業はこの change に含めない

## Capabilities

### New Capabilities

- `reference-score-improvement`: Draw.io / Mermaid の既知 score 未達を改善し、全 supported pattern を 99 点以上へ上げる
- `reference-score-reporting`: 改善前後の score と contact sheet を review できる形で残す

## Impact

- `crates/katana-canvas-forge/src/markdown/drawio_renderer/` — Draw.io renderer / resource resolver / SVG postprocess
- `scripts/drawio/` — score 比較、report、baseline policy
- `scripts/mermaid/` — supported fixture の score 改善、accepted score floor policy
- `tests/fixtures/drawio/representative/score-baseline.json` — 既知下限の改善
- `tests/fixtures/drawio/` — score 改善対象 fixture と reference
- `openspec/changes/v0-1-4-reference-score-improvement/` — 本 change の仕様とタスク
