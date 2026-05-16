## ADDED Requirements

### Requirement: TypeScript import 境界は package imports 前提で検査されなければならない

システムは、runtime TypeScript source の import 境界を検査し、`package.json` `imports` に定義された `#shared/*`、`#mermaid/*`、`#drawio/*`、`#zenuml/*` 形式の subpath imports を正規経路として強制しなければならない（MUST）。`source/shared` から runtime 固有領域への依存、Mermaid / Draw.io / ZenUML の相互直接依存、領域またぎ相対 import は失敗扱いにしなければならない（MUST）。

#### Scenario: `#` import 境界を検査する

- **WHEN** `just ast-lint` 相当の構造検査を実行する
- **THEN** `diagram_runtime/source/**/*.ts` の領域またぎ import は `#shared/...`、`#mermaid/...`、`#drawio/...`、`#zenuml/...` のみ許可される
- **THEN** `../shared/...` のような領域またぎ相対 import がある場合は失敗する
- **THEN** `@shared/...` のような独自 alias がある場合は失敗する
- **THEN** `#/shared/...` のような slash あり subpath imports がある場合は失敗する

#### Scenario: Runtime 間の直接依存を検査する

- **WHEN** `just ast-lint` 相当の構造検査を実行する
- **THEN** `source/shared` は `source/mermaid`、`source/drawio`、`source/zenuml` に依存しない
- **THEN** Mermaid / Draw.io / ZenUML は相互に直接 import しない

### Requirement: Bundle toolchain 設定は品質 gate で検査されなければならない

システムは、bundle toolchain が ESM graph、package `imports`、TypeScript 変換、minify / mangle を扱う構成であることを `runtime-bundle-check` で検査しなければならない（MUST）。Terser 単体で bundle を構成している場合は失敗扱いにしなければならない（MUST）。

#### Scenario: Bundle toolchain を検査する

- **WHEN** `just runtime-bundle-check` を実行する
- **THEN** Rollup または同等の bundler が ESM graph を解決していることを確認できる
- **THEN** `@rollup/plugin-node-resolve` または同等の resolver が package `imports` を解決していることを確認できる
- **THEN** Rollup output が V8 通常 script として評価できる `iife` 形式であることを確認できる
- **THEN** Terser は minify / mangle stage として使われていることを確認できる
- **THEN** Terser 単体で `#` import 解決をしている構成は失敗する

### Requirement: Generated bundle は minify / mangle 済みであることを検査されなければならない

システムは、生成済み `*-runtime.min.js` が実際に minify / mangle された artifact であることを検査できなければならない（MUST）。検査は入口 I/F を壊さず、内部実装の整形済み未圧縮 bundle が `*.min.js` として混入することを検出しなければならない（MUST）。

#### Scenario: Minified bundle を検査する

- **WHEN** `just runtime-bundle-check` を実行する
- **THEN** 生成済み `mermaid-runtime.min.js`、`drawio-runtime.min.js`、`zenuml-runtime.min.js` は再生成結果と一致する
- **THEN** minify / mangle stage を通らない生成物との差分を検出できる
- **THEN** entry I/F の `katanaRunMermaidRuntime`、`katanaRunDrawioRuntime`、`katanaRunZenumlRuntime` は保持されている
- **THEN** `katanaInstallMermaidZenumlRuntimeAdapter` は Rust 側から呼ぶ外部 entry I/F として要求されない

### Requirement: Rust/V8 entry I/F の保護を検査しなければならない

システムは、Rust 側が呼ぶ runtime entry I/F が bundle と render script の両方で一致していることを検査しなければならない（MUST）。Entry I/F を変更する場合は公開 renderer API と同等の扱いで OpenSpec 更新を要求しなければならない（MUST）。

#### Scenario: Entry I/F を検査する

- **WHEN** Rust runtime tests または AST lint を実行する
- **THEN** Rust 側 render script が呼ぶ entry 名と generated bundle が公開する entry 名が一致する
- **THEN** Terser reserved name または `globalThis["..."]` により entry 名が保護されている
- **THEN** entry 名が暗黙の関数宣言だけに依存する場合は失敗する
- **THEN** Rust 側 render script が `katanaInstallMermaidZenumlRuntimeAdapter()` を直接呼ぶ場合は失敗する
