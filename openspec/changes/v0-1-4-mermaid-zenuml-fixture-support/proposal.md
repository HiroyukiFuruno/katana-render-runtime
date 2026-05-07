## Why

v0.1.0 の Mermaid full compare では `28-zen-uml.md` が `UnknownDiagramError` になり、さらに空出力に近い case で比較処理が null 参照を起こした。

これは初回 release を止める問題ではないが、移行計画から漏れていた Mermaid unsupported diagram / ZenUML fixture の扱いである。v0.1.4 で、対応方針を明示して score 実行を壊さない形にする。

## What Changes

- `28-zen-uml.md` を supported にするか、unsupported fixture として明示的に扱うかを決める
- unsupported diagram は error first で理由を report に残す
- compare が空 SVG / 未生成 PNG で null 参照しないようにする
- full compare の対象選定に、暗黙 skip ではない fixture metadata を導入する

## Non-Goals

- v0.1.2 の Draw.io / Mermaid score 改善に混ぜない
- Mermaid.js / Draw.io.js の version 固定は v0.1.1 に送る
- export CSS 回帰修正と macOS debug open は v0.1.3 に送る
- fallback SVG で ZenUML を描けた扱いにしない

## Capabilities

### New Capabilities

- `mermaid-unsupported-fixture-policy`: unsupported diagram を明示的に report へ出す
- `mermaid-zenuml-fixture-support`: ZenUML fixture の support / unsupported 境界を固定する

## Impact

- `tests/fixtures/mermaid/` — ZenUML fixture metadata、unsupported reason
- `scripts/mermaid/` — full compare の対象選定と error report
- `crates/katana-canvas-forge/src/markdown/mermaid_renderer/` — 必要な場合のみ ZenUML runtime 対応
- `openspec/changes/v0-1-4-mermaid-zenuml-fixture-support/` — 本 change の仕様とタスク
