## ADDED Requirements

### Requirement: Runtime TypeScript source は ESM と package imports で依存関係を表現しなければならない

システムは、`crates/katana-diagram-renderer/src/markdown/diagram_runtime/source/` 配下の runtime TypeScript source を ESM（ECMAScript Modules）として管理しなければならない（MUST）。Runtime source の領域またぎ import は、`package.json` の `imports` に定義した `#shared/*`、`#mermaid/*`、`#drawio/*`、`#zenuml/*` 形式の subpath imports を使わなければならない（MUST）。`@shared/*`、`#/shared/*` のような別形式や、領域またぎの相対 import を正規経路にしてはならない（MUST NOT）。

#### Scenario: Shared helper を import する

- **WHEN** Mermaid runtime source が shared helper を参照する
- **THEN** code は `import ... from "#shared/..."` 形式を使う
- **THEN** `package.json` の `imports` に `#shared/*` の解決先が定義されている
- **THEN** `../shared/...` または `@shared/...` 形式を正規 import として使わない

#### Scenario: Runtime 固有 entrypoint を import する

- **WHEN** bundle entry が runtime 固有 module を参照する
- **THEN** Mermaid は `#mermaid/...`、Draw.io は `#drawio/...`、ZenUML は `#zenuml/...` を使う
- **THEN** Mermaid / Draw.io / ZenUML の相互直接 import は存在しない

### Requirement: Bundle toolchain は ESM graph と package imports を解決しなければならない

システムは、Runtime TypeScript source の ESM graph と `package.json` `imports` を bundle 時に解決しなければならない（MUST）。Bundle toolchain の第一候補は Rollup、`@rollup/plugin-node-resolve`、TypeScript 変換 plugin、Terser とし、Terser 単体を bundle tool として扱ってはならない（MUST NOT）。

#### Scenario: Bundle を生成する

- **WHEN** 開発者が runtime bundle 生成 recipe を実行する
- **THEN** Rollup は `package.json` `imports` の `#shared/*` 形式の subpath imports を解決する
- **THEN** TypeScript source は JavaScript へ変換される
- **THEN** `mermaid-runtime.min.js`、`drawio-runtime.min.js`、`zenuml-runtime.min.js` が生成される
- **THEN** Rollup output は V8 通常 script として評価できる `iife` 形式である
- **THEN** Terser は生成済み JavaScript に minify と mangle を適用する

#### Scenario: Terser 単体では bundle 要件を満たさない

- **WHEN** 実装者が bundle tool を選定する
- **THEN** TypeScript transpile、ESM graph 解決、`package.json` `imports` 解決が必要な要件として確認される
- **THEN** Terser は minify / mangle 用 tool として扱われる
- **THEN** Terser 単体を `#` import 解決または ESM bundle の責務にしない

### Requirement: 生成済み bundle は Rust V8 が通常 script として実行できなければならない

システムは、bundle 後の `*-runtime.min.js` を Rust 側の V8 が `v8::Script::compile` で通常 script として評価できる自己完結 JavaScript にしなければならない（MUST）。生成済み bundle は V8 実行時に `import` / `export`、import map、module resolver、Rollup、Bun、Node、Deno、TypeScript compiler を要求してはならない（MUST NOT）。

#### Scenario: Mermaid runtime bundle を V8 で実行する

- **WHEN** `MermaidRenderer` が `mermaid-runtime.min.js` を V8 へ渡す
- **THEN** bundle は `import` / `export` を含まない
- **THEN** V8 実行時に module resolution を要求しない
- **THEN** Rust 側は bundle とは別に TypeScript source を読み込まない

#### Scenario: Draw.io runtime bundle を V8 で実行する

- **WHEN** `DrawioRenderer` が `drawio-runtime.min.js` を V8 へ渡す
- **THEN** bundle は自己完結している
- **THEN** shared helper を別 script として Rust 側から追加読み込みしない

### Requirement: Rust から呼ぶ runtime entry I/F を維持しなければならない

システムは、minify / mangle / 難読化後も Rust 側の render script が呼ぶ entry I/F を維持しなければならない（MUST）。Entry I/F は `globalThis["katanaRunMermaidRuntime"]`、`globalThis["katanaRunDrawioRuntime"]`、`globalThis["katanaRunZenumlRuntime"]` のような固定 property として公開しなければならない（MUST）。ZenUML 登録用の補助関数は Rust 側 render script から直接呼ぶ外部 I/F にしてはならない（MUST NOT）。

#### Scenario: Mermaid entry を呼び出す

- **WHEN** Rust 側の render script が `katanaRunMermaidRuntime(requestJson)` を実行する
- **THEN** minify / mangle 後の bundle でも同じ entry が存在する
- **THEN** entry 名は Terser の mangle により変更されない
- **THEN** Rust 側 render script は `katanaInstallMermaidZenumlRuntimeAdapter()` を直接呼ばない

#### Scenario: ZenUML entry を呼び出す

- **WHEN** Rust 側の render script が `katanaRunZenumlRuntime(source, isDark)` を実行する
- **THEN** minify / mangle 後の bundle でも同じ entry が存在する
- **THEN** entry は generated `zenuml-runtime.min.js` から供給される
