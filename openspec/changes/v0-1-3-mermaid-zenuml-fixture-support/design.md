## Context

Mermaid full compare は、公式 Mermaid が認識しない diagram type を含む fixture で失敗する。

確認済みの失敗:

- `tests/fixtures/mermaid/en/28-zen-uml.md`
- `tests/fixtures/mermaid/ja/28-zen-uml.md`
- `UnknownDiagramError`
- rasterize / compare 側の null 参照

v0.1.3 は、ZenUML を実際に support するか、unsupported として明示的に扱うかを固定する change である。

## Goals

- unsupported diagram を暗黙 skip しない
- compare が null 参照で落ちず、fixture 名と理由を出す
- supported fixture と unsupported fixture の境界を metadata で管理する
- fallback SVG を禁止する

## Policy

最初に Mermaid.js の取り込み version と plugin 状態を確認する。

ZenUML が公式 runtime で support 可能なら、runtime 初期化に必要な設定を実装し、fixture を supported にする。

support できない場合は、fixture metadata に unsupported reason を記録し、full compare report に「未対応」として出す。この場合も、compare 自体は panic / null 参照ではなく error first の結果として扱う。

## Verification

- `mermaid-compare-full` が null 参照で落ちない
- `28-zen-uml.md` の扱いが supported / unsupported のどちらかで明示される
- unsupported の場合、reason が report に残る
- fallback SVG / stub PNG が追加されていない
- `just check`
- `npx -y @fission-ai/openspec validate "v0-1-3-mermaid-zenuml-fixture-support" --strict`
