# mermaid-zenuml-rendering Specification

## Purpose
Mermaid の ZenUML diagram を kcf の通常描画・参照生成・比較評価の対象として扱うための仕様を定義する。
## Requirements
### Requirement: ZenUML diagram を描画しなければならない

システムは、Mermaid コードブロック内の diagram type が `zenuml` の場合、ZenUML 対応JSを使って SVG を描画しなければならない（MUST）。`zenuml` を未対応 fixture として score 対象から外してはならない（MUST NOT）。

#### Scenario: ZenUML fixture を描画する

- **GIVEN** `tests/fixtures/mermaid/en/28-zen-uml.md` または `tests/fixtures/mermaid/ja/28-zen-uml.md` の Mermaid コードブロック
- **WHEN** full compare を実行する
- **THEN** renderer は diagram type を `zenuml` と判定する
- **THEN** renderer は ZenUML 対応JSを Mermaid runtime に登録する
- **THEN** renderer は `UnknownDiagramError` を返さず SVG を生成する
- **THEN** full compare は `28-zen-uml.md` を score 対象に含める

### Requirement: ZenUML 対応JSを固定 asset として管理しなければならない

システムは、ZenUML 対応JSを repository 管理の固定 asset として扱い、version と checksum を管理しなければならない（MUST）。

#### Scenario: ZenUML asset を materialize する

- **GIVEN** runtime asset の初期化が実行される
- **WHEN** Mermaid renderer が ZenUML 対応JSを必要とする
- **THEN** `crates/katana-canvas-forge/vendor/mermaid-zenuml/<version>/mermaid-zenuml.min.js` 由来の asset が materialize される
- **THEN** materialize 前後で checksum が一致する
- **THEN** 外部ネットワーク取得なしで render runtime に読み込める

### Requirement: ZenUML 外部 diagram を render 前に登録しなければならない

システムは、`zenuml` diagram を render する前に、ZenUML 対応JSを `mermaid.registerExternalDiagrams` 相当の API で登録しなければならない（MUST）。

#### Scenario: ZenUML render script を実行する

- **GIVEN** `katanaMermaidDiagramType()` が `zenuml` を返す source
- **WHEN** `render_mermaid.js` が SVG render を開始する
- **THEN** `globalThis.__katanaMermaidZenuml` が存在することを確認する
- **THEN** `mermaidValue.registerExternalDiagrams([globalThis.__katanaMermaidZenuml])` 相当の登録を render 前に実行する
- **THEN** 登録できない場合は fallback SVG を生成せず error を返す

### Requirement: 公式参照生成も ZenUML 対応JSを使わなければならない

システムは、Mermaid 公式参照生成でも Katana runtime と同じ ZenUML 対応JSを読み込み、`28-zen-uml.md` の参照SVG / PNGを生成しなければならない（MUST）。

#### Scenario: ZenUML の参照画像を更新する

- **GIVEN** `diagram-update` が `28-zen-uml.md` を処理する
- **WHEN** 公式参照SVG / PNGを生成する
- **THEN** 公式 renderer は ZenUML 対応JSを読み込む
- **THEN** 公式 renderer は ZenUML 外部 diagram を登録する
- **THEN** 生成された参照は fallback SVG / stub PNG ではない
