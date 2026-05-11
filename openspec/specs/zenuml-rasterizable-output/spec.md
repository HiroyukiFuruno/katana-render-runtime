# zenuml-rasterizable-output Specification

## Purpose
ZenUML の `MermaidRenderer` 出力が、KatanA 相当のネイティブ画像化経路（resvg/usvg）で非空・非白になることを保証する。`foreignObject` を含む SVG をそのまま返さない。

## Requirements

### Requirement: ZenUML 出力は foreignObject を含まない SVG でなければならない

システムは、ZenUML diagram の render 結果として `<foreignObject` 要素を含まない SVG 文字列を返さなければならない（MUST）。返す SVG は `<svg` ルート要素と `<image` 要素を含む形式でなければならない（MUST）。

#### Scenario: ZenUML SVG に foreignObject が含まれない

- **WHEN** `MermaidRenderer` または `MermaidJsRuntimeOps::render` に `zenuml` ダイアグラムソースを渡す
- **THEN** 返り値の SVG 文字列に `<foreignObject` が含まれない
- **THEN** 返り値の SVG 文字列に `<image` が含まれる
- **THEN** 返り値の SVG 文字列に `data:image/png;base64,` が含まれる

### Requirement: ZenUML 出力は resvg/usvg 相当のネイティブ画像化経路で非空・非白でなければならない

システムは、ZenUML の SVG 出力を resvg/usvg 相当のラスタライズ処理に渡した場合に、非空（empty でない）かつ非白（全ピクセルが白色でない）の画像を生成できる SVG を返さなければならない（MUST）。

#### Scenario: ネイティブ画像化経路で非白画像になる

- **GIVEN** `SvgRasterizeOps` 相当の処理（foreignObject 削除を含む）
- **WHEN** ZenUML の `MermaidRenderer` 出力を渡す
- **THEN** ラスタライズ結果が空でない
- **THEN** ラスタライズ結果の全ピクセルが白色（RGBA #FFFFFF）でない

### Requirement: ZenUML bounding box が 0 の場合はエラーを返さなければならない

システムは、Playwright 上で ZenUML diagram の bounding box の width または height が 0 以下の場合、描画成功として空または白の SVG を返してはならない（MUST NOT）。代わりに `DiagramResult::Err` を返さなければならない（MUST）。

#### Scenario: bounding box が取れない場合はエラー

- **WHEN** `#diagram` 要素の bounding box の width または height が 0 以下
- **THEN** `DiagramResult::Ok` を返さない
- **THEN** `DiagramResult::Err` を返す
