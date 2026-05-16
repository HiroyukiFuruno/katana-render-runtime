## Why

KDR の runtime source は TypeScript 化されたものの、依存関係はまだ明示的な ESM import ではなく、bundle 定義側の順序付き断片一覧に強く依存している。保守者や AI agent が依存関係を追うには、`export` / `import` と `package.json` の `imports` による明示的な module graph へ移す必要がある。

また、生成物名は `*.min.js` だが、現状の bundle 生成は主に結合であり、実際の minify / mangle / 難読化と、ZenUML 専用 bundle を実ロジックで使う契約が不十分である。

## What Changes

- Runtime TypeScript source を ESM（ECMAScript Modules: `export` / `import` を使う JavaScript 標準のモジュール形式）へ移行する
- Runtime TypeScript source の内部 import は相対パスではなく、`package.json` の `imports` に定義した `#shared/*`、`#mermaid/*`、`#drawio/*`、`#zenuml/*` 形式の subpath imports を使う
- `@shared/...` のような独自 alias ではなく、Node.js / TypeScript の `package.json` `imports` を正とする
- `tsconfig.json` は `package.json` `imports` を解決できる `moduleResolution: "bundler"` または同等設定を使う
- Bundle toolchain は ESM graph と `imports` を解決できる bundler を主役にし、Terser 単体を bundle tool として扱わない
- 第一候補 tech stack は Rollup + `@rollup/plugin-node-resolve` + TypeScript 変換 plugin + Terser とする
- 出力は `mermaid-runtime.min.js`、`drawio-runtime.min.js`、`zenuml-runtime.min.js` の3 bundle を維持する
- 出力 bundle は Rust の V8 が `v8::Script::compile` で通常 script として評価できる自己完結 JavaScript にする
- Rust から呼ぶ入口 I/F（interface: 呼び出し口の約束）は `globalThis["katanaRun..."]` で固定し、minify / mangle / 難読化で壊さない。既存の ZenUML 登録用補助関数は外部入口ではなく bundle 内部責務へ寄せる
- 生成済み `*.min.js` には実際の minify / mangle / 難読化を適用する
- ZenUML 専用 runtime bundle をテスト限定ではなく、実描画経路のロジックで使う
- 実行時に TypeScript compiler、Rollup、Bun、Node、Deno、module resolver、import map を要求しない
- 他 agent が迷わないよう、Terser 単体を採らない理由、`imports` の `#` 制約、Rust/V8 の入口 I/F 維持を design と tasks に明文化する

## Capabilities

### New Capabilities

- なし

### Modified Capabilities

- `diagram-runtime-bundle-management`: Runtime source を ESM + `package.json` `imports` で管理し、Rollup 系 bundler で自己完結 `*.min.js` を生成する契約を追加する
- `runtime-code-quality-gates`: `#shared/*` 形式の subpath imports、ESM 境界、相対パス禁止、入口 I/F 保護、minify / mangle / 難読化済み bundle の同期を検査する契約を追加する
- `mermaid-zenuml-rendering`: ZenUML 専用 `zenuml-runtime.min.js` を実描画経路で使う契約を追加する
- `runtime-asset-versioning`: 生成済み bundle checksum を、minify / mangle / 難読化後の最終成果物として固定する契約を追加する

## Impact

- `package.json`
- `tsconfig.json`
- `biome.json`
- Rollup / Terser 等の bundle toolchain 設定
- `scripts/runtime-bundles/bundle-runtime.ts`
- `crates/katana-diagram-renderer/src/markdown/diagram_runtime/source/{shared,mermaid,drawio,zenuml}/`
- `crates/katana-diagram-renderer/src/markdown/diagram_runtime/generated/`
- `crates/katana-diagram-renderer/src/markdown/mermaid_renderer/js_runtime_scripts.rs`
- `crates/katana-diagram-renderer/src/markdown/drawio_renderer/js_runtime_scripts.rs`
- `crates/katana-diagram-renderer/src/markdown/mermaid_renderer/zenuml_v8_runtime.rs`
- `crates/katana-diagram-renderer/src/markdown/diagram_js_runtime.rs`
- `crates/kdr-linter/src/rules/runtime_bundles/`
- Runtime bundle checksum manifest
- `Justfile` の runtime bundle 生成・検証・package 検証 recipe
