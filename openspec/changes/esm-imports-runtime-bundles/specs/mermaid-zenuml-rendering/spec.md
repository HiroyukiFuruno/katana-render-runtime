## ADDED Requirements

### Requirement: ZenUML production rendering は generated ZenUML runtime bundle を使わなければならない

システムは、ZenUML の production rendering path で generated `zenuml-runtime.min.js` を読み込まなければならない（MUST）。`zenuml-runtime.min.js` はテスト専用 helper ではなく、実描画で使う KDR runtime adapter として扱われなければならない（MUST）。Mermaid 本体 bundle の暗黙副作用だけに ZenUML 登録を依存させてはならない（MUST NOT）。

#### Scenario: ZenUML diagram を実描画する

- **WHEN** renderer が `zenuml` diagram を描画する
- **THEN** Rust 側は generated `zenuml-runtime.min.js` を V8 へ渡す
- **THEN** ZenUML vendor asset は固定 version / checksum の asset として読み込まれる
- **THEN** `katanaRunZenumlRuntime(source, isDark)` は generated bundle から供給される
- **THEN** 生成 SVG は既存の ZenUML rasterizable output 要件を満たす

#### Scenario: Mermaid 経由の ZenUML 登録を行う

- **WHEN** Mermaid renderer が diagram type `zenuml` を検出する
- **THEN** Rust 側 script assembly は `zenuml-runtime.min.js` を `mermaid-zenuml.min.js` の後、render script の前に追加する
- **THEN** ZenUML 登録に必要な KDR adapter は `zenuml-runtime.min.js` の責務として扱われる
- **THEN** Mermaid 本体 `mermaid-runtime.min.js` へ ZenUML 専用 adapter を暗黙統合しない
- **THEN** Rust 側 render script は `katanaInstallMermaidZenumlRuntimeAdapter()` を直接呼ばない
- **THEN** 登録順序は focused runtime test で検証される
