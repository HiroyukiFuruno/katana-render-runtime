## Why

kdr v0.1.3 の ZenUML 出力は `foreignObject` を含む SVG を返すが、KatanA の preview pipeline は `resvg`/`usvg` でラスタライズする前に `foreignObject` を削除するため、ZenUML の描画内容がすべて消えて白表示になる。通常の Mermaid / Draw.io は `foreignObject` を含まない SVG を返しているため consumer 側では問題が発生しておらず、ZenUML だけが例外的な出力形式を持っている。

## What Changes

- ZenUML の `MermaidRenderer` 出力を `foreignObject` なしで native rasterization（resvg/usvg）に通せる形式へ変更する
- 出力形式の選択肢は「`foreignObject` を含まない SVG」または「PNG バイト列を output contract に乗せる」のいずれかとし、design フェーズで決定する
- ZenUML の既存テストを、KatanA 相当のネイティブ画像化経路で非空・非白になることを確認する形に更新する
- `mermaid-zenuml-rendering` spec に output format 契約を追加する（`foreignObject` を含まないこと、または PNG output 形式を明記すること）

## Capabilities

### New Capabilities

- `zenuml-rasterizable-output`: ZenUML の `MermaidRenderer` 出力が `resvg`/`usvg` 相当のネイティブ画像化経路で非空・非白になることを保証する。`foreignObject` を含む SVG をそのまま返さないこと、または PNG バイト列を返す output contract を明示すること。

### Modified Capabilities

- `mermaid-zenuml-rendering`: ZenUML が SVG を生成できること（既存要件）に加え、出力が `foreignObject` を含まない、またはネイティブ画像化 consumer が別扱いなしに処理できる形式であるという output format 要件を追加する。

## Impact

- `src/markdown/mermaid_renderer/` — ZenUML render path、出力 SVG postprocess または PNG 変換
- `src/markdown/mermaid_renderer/js_runtime_tests.rs` — ZenUML テストの期待値を output format 契約に合わせて更新
- `openspec/specs/mermaid-zenuml-rendering/spec.md` — output format 要件追加（delta spec）
- `openspec/specs/zenuml-rasterizable-output/spec.md` — 新規 spec 追加
