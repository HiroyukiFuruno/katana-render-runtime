## MODIFIED Requirements

### Requirement: ZenUML 対応JSを固定 asset として管理しなければならない

システムは、ZenUML 対応JSを repository 管理の固定 asset として扱い、version と checksum を管理しなければならない（MUST）。ZenUML の KCF runtime adapter は Mermaid 本体 bundle に暗黙で混ぜず、TypeScript source から生成される独立した `zenuml-runtime.min.js` として管理しなければならない（MUST）。

#### Scenario: ZenUML asset を materialize する

- **GIVEN** runtime asset の初期化が実行される
- **WHEN** Mermaid renderer が ZenUML 対応JSを必要とする
- **THEN** `crates/katana-canvas-forge/vendor/mermaid-zenuml/<version>/mermaid-zenuml.min.js` 由来の asset が materialize される
- **THEN** materialize 前後で checksum が一致する
- **THEN** 外部ネットワーク取得なしで render runtime に読み込める
- **THEN** KCF 生成 `zenuml-runtime.min.js` を Mermaid 本体 bundle とは別 artifact として検証できる

### Requirement: ZenUML 外部 diagram を render 前に登録しなければならない

システムは、`zenuml` diagram を render する前に、ZenUML 対応JSを `mermaid.registerExternalDiagrams` 相当の API で登録しなければならない（MUST）。登録に必要な KCF adapter は `zenuml-runtime.min.js` から供給され、Mermaid runtime bundle の暗黙副作用に依存してはならない（MUST NOT）。

#### Scenario: ZenUML render script を実行する

- **GIVEN** `katanaMermaidDiagramType()` が `zenuml` を返す source
- **WHEN** `render_mermaid.js` 相当の bundle entrypoint が SVG render を開始する
- **THEN** `globalThis.__katanaMermaidZenuml` が存在することを確認する
- **THEN** `mermaidValue.registerExternalDiagrams([globalThis.__katanaMermaidZenuml])` 相当の登録を render 前に実行する
- **THEN** 登録できない場合は fallback SVG を生成せず error を返す
- **THEN** 登録に必要な KCF adapter は `zenuml-runtime.min.js` の読み込み結果として提供される
