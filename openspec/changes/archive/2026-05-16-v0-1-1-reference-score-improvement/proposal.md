## Why

v0.1.0 は初回移植版として release するが、KatanA 側へまだ取り込まないため、既存品質は劣化しない。

一方で、移植時の full compare では Draw.io official diagrams の `i18n` と `link` が 99 点に届かず、代表ケースにも既知下限が残っている。これは v0.1.0 の公開を止めるのではなく、v0.1.x の最終フェーズで扱う。

この範囲は旧計画で複数フェーズに分かれていたが、粒度が大きく進捗が停滞している。初回公開を `v0.1.0` へ戻したため、score 改善（score improvement）は直後の `v0.1.1` で扱う。不要になった export CSS debug 計画は KDR release 番号から外し、旧 export/debug 論点は KDV 移譲記録で扱う。

v0.1.1 の対象は CI 代表ケースだけではなく、full compare が見る全 supported pattern である。
当初は Draw.io / Mermaid ともに 99 点以上を完了基準としていたが、実装後の全量評価で 95 点以上なら主要な図形、文字、接続線、全体構図が機能する状態に到達していることを確認した。

そのため v0.1.1 は 95 点以上を release 下限とし、「公式をなるべく忠実に踏襲」する版として release する。
99 点以上は「公式を完全踏襲」と言える水準として後続 change へ送る。

## What Changes

- Draw.io / Mermaid の full compare 対象にある全 supported pattern の score 未達ケースを調査し、描画差分の原因を分類する
- 修正できる描画差分は renderer / resource resolver / postprocess で改善する
- 95 点未満の致命的な未達を解消し、95〜99 点未満の残差を正式 report に残す
- Mermaid の supported fixture も full compare で 95 点以上を満たす
- unsupported fixture を暗黙 skip しない
- score report と contact sheet を PR review で確認できる成果物として残す

## Non-Goals

- v0.1.0 release を延期しない
- Mermaid ZenUML の新規文法対応や機能拡張はしない
- Mermaid.js / Draw.io.js の version 固定と取り込み更新は別 change に送る
- 不要になった export CSS debug 計画は KDR release 番号から外し、旧 export/debug 論点は KDV 移譲記録で扱う
- KatanA 側への取り込み（consume）作業はこの change に含めない

## Capabilities

### New Capabilities

- `reference-score-improvement`: Draw.io / Mermaid の既知 score 未達を改善し、全 supported pattern を 95 点以上へ上げる
- `reference-score-reporting`: 改善前後の score と contact sheet を review できる形で残す

## Impact

- `crates/katana-diagram-renderer/src/markdown/drawio_renderer/` — Draw.io renderer / resource resolver / SVG postprocess
- `scripts/drawio/` — score 比較、report、baseline policy
- `scripts/mermaid/` — supported fixture の score 改善、accepted score floor policy
- `tests/fixtures/drawio/representative/score-baseline.json` — 既知下限の改善
- `tests/fixtures/drawio/` — score 改善対象 fixture と reference
- `openspec/changes/v0-1-1-reference-score-improvement/` — 本 change の仕様とタスク
