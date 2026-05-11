# mermaid-zenuml-rendering Delta Specification

## MODIFIED Requirements

### Requirement: ZenUML diagram を描画しなければならない

システムは、Mermaid コードブロック内の diagram type が `zenuml` の場合、ZenUML 対応JSを使って SVG を描画しなければならない（MUST）。`zenuml` を未対応 fixture として score 対象から外してはならない（MUST NOT）。描画結果の SVG は `<foreignObject` を含んではならない（MUST NOT）。描画結果の SVG は `<image` 要素と `data:image/png;base64,` を含まなければならない（MUST）。

#### Scenario: ZenUML fixture を描画する

- **GIVEN** `tests/fixtures/mermaid/en/28-zen-uml.md` または `tests/fixtures/mermaid/ja/28-zen-uml.md` の Mermaid コードブロック
- **WHEN** full compare を実行する
- **THEN** renderer は diagram type を `zenuml` と判定する
- **THEN** renderer は ZenUML 対応JSを Mermaid runtime に登録する
- **THEN** renderer は `UnknownDiagramError` を返さず SVG を生成する
- **THEN** 生成された SVG に `<foreignObject` が含まれない
- **THEN** 生成された SVG に `<image` と `data:image/png;base64,` が含まれる
- **THEN** full compare は `28-zen-uml.md` を score 対象に含める
