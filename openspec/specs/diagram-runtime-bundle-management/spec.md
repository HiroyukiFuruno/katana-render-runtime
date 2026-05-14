# diagram-runtime-bundle-management Specification

## Purpose
TBD - created by archiving change typescript-diagram-runtime-bundles. Update Purpose after archive.
## Requirements
### Requirement: V8 へ渡す runtime code は生成済み JavaScript bundle でなければならない

システムは、Mermaid / Draw.io / ZenUML の KCF runtime adapter を TypeScript source で管理しても、V8 へ渡す code は生成済み JavaScript bundle でなければならない（MUST）。V8 実行時に TypeScript source、type stripping、transpile、module resolution を要求してはならない（MUST NOT）。

#### Scenario: Mermaid runtime を実行する

- **WHEN** `MermaidRenderer` が V8 runtime を起動する
- **THEN** Rust 側は生成済み `mermaid-runtime.min.js` を読み込む
- **THEN** V8 へ TypeScript source を渡さない
- **THEN** V8 実行中に TypeScript compiler、Rollup、Bun、Node、Deno を起動しない

#### Scenario: Draw.io runtime を実行する

- **WHEN** `DrawioRenderer` が V8 runtime を起動する
- **THEN** Rust 側は生成済み `drawio-runtime.min.js` を読み込む
- **THEN** V8 へ TypeScript source を渡さない
- **THEN** V8 実行中に module import 解決を行わない

### Requirement: Runtime bundle は Mermaid / Draw.io / ZenUML ごとに分離されなければならない

システムは、KCF runtime adapter の生成物を Mermaid / Draw.io / ZenUML ごとの独立した JavaScript bundle として管理しなければならない（MUST）。1つの巨大な `index-min.js` に全 runtime を暗黙統合してはならない（MUST NOT）。

#### Scenario: 合意済み階層で TypeScript source を配置する

- **WHEN** runtime TypeScript source を配置する
- **THEN** shared helper は `crates/katana-canvas-forge/src/markdown/diagram_runtime/source/shared/` に置かれる
- **THEN** Mermaid runtime source は `crates/katana-canvas-forge/src/markdown/diagram_runtime/source/mermaid/` に置かれる
- **THEN** Draw.io runtime source は `crates/katana-canvas-forge/src/markdown/diagram_runtime/source/drawio/` に置かれる
- **THEN** ZenUML runtime source は `crates/katana-canvas-forge/src/markdown/diagram_runtime/source/zenuml/` に置かれる
- **THEN** generated bundle は `crates/katana-canvas-forge/src/markdown/diagram_runtime/generated/` に置かれる

#### Scenario: Runtime bundle を生成する

- **WHEN** 開発者が runtime bundle 生成 recipe を実行する
- **THEN** `mermaid-runtime.min.js` が生成される
- **THEN** `drawio-runtime.min.js` が生成される
- **THEN** `zenuml-runtime.min.js` が生成される
- **THEN** それぞれの bundle は対応 runtime の entrypoint を持つ

#### Scenario: Mermaid だけを描画する

- **WHEN** Mermaid diagram を描画する
- **THEN** Draw.io 専用 runtime bundle を読み込まない
- **THEN** ZenUML 専用 runtime bundle の読み込み有無は diagram type 判定または登録順序の仕様に従う

### Requirement: Shared helper は TypeScript source 上で共有され、V8 実行時には自己完結しなければならない

システムは、browser / DOM / SVG helper を TypeScript source 上で共有できるが、生成済み bundle は V8 実行時に外部 module resolver を必要としない自己完結した JavaScript でなければならない（MUST）。

#### Scenario: Shared helper を使う bundle を実行する

- **WHEN** V8 が生成済み runtime bundle を compile する
- **THEN** bundle は `import` / `require` による外部 file 解決を要求しない
- **THEN** shared helper の不足により runtime error を返さない
- **THEN** Rust 側の script 順序へ shared helper の読み込みを分散させない

### Requirement: 生成済み bundle は repository と package に含めなければならない

システムは、生成済み JavaScript bundle を repository 管理し、crates.io package に含めなければならない（MUST）。Crate 利用者の build 時に JavaScript toolchain を要求してはならない（MUST NOT）。

#### Scenario: Crate package を作成する

- **WHEN** `cargo package -p katana-canvas-forge --locked --allow-dirty --list` 相当の package 内容確認を実行する
- **THEN** Mermaid / Draw.io / ZenUML の生成済み runtime bundle が package に含まれる
- **THEN** TypeScript source を package に含めるかどうかは明示的な include / exclude 方針に従う
- **THEN** package 利用者の `cargo build` は Rollup、Bun、Node、Deno を要求しない

### Requirement: TypeScript source と生成済み bundle の同期を検証しなければならない

システムは、TypeScript source から runtime bundle を再生成した結果が repository 管理済み bundle と一致することを検証できなければならない（MUST）。

#### Scenario: Bundle 同期検証を実行する

- **WHEN** 開発者または CI が bundle 同期検証 recipe を実行する
- **THEN** TypeScript source から Mermaid / Draw.io / ZenUML bundle を再生成する
- **THEN** 再生成結果と repository 管理済み bundle の差分を検出する
- **THEN** 差分がある場合は検証を失敗させる

### Requirement: Runtime bundle 化は公開 renderer API を変更してはならない

システムは、runtime adapter を TypeScript bundle 化しても `Renderer`、`RenderInput`、`RenderOutput`、CLI の公開 contract を変更してはならない（MUST NOT）。

#### Scenario: Consumer が既存 API で描画する

- **WHEN** consumer が既存の `Renderer` API で Mermaid または Draw.io を描画する
- **THEN** runtime bundle 化前と同じ公開型で結果を受け取る
- **THEN** consumer は TypeScript source、bundle path、JavaScript toolchain を指定しない
- **THEN** KatanA UI state、preview state、workspace state を要求されない
