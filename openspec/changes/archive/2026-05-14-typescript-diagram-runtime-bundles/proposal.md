## Why

KCF の V8 runtime は多数の手書き JavaScript を順番に評価しており、型のない結合と評価回数の多さが保守性と起動時の無駄になっている。

V8 に TypeScript を直接実行させるのではなく、開発元を TypeScript に寄せ、配布物として deterministic な JavaScript bundle を同梱することで、管理性と runtime 評価効率を上げる。

## What Changes

- Mermaid / Draw.io / ZenUML の KCF runtime adapter を TypeScript source で管理する
- TypeScript source から生成済み JavaScript bundle を作り、Rust 側は生成済み bundle だけを `include_str!` で読み込む
- bundle は `mermaid-runtime.min.js`、`drawio-runtime.min.js`、`zenuml-runtime.min.js` に分離する
- Mermaid / Draw.io / ZenUML が共有する browser / DOM / SVG helper は TypeScript 側で共有し、出力 bundle では各 runtime の独立性を保つ
- ユーザー合意済みの階層として、TypeScript source は `crates/katana-canvas-forge/src/markdown/diagram_runtime/source/{shared,mermaid,drawio,zenuml}`、生成物は `crates/katana-canvas-forge/src/markdown/diagram_runtime/generated/` 配下の runtime 別 bundle に分ける
- V8 実行時に TypeScript transpile、type stripping、module resolution、外部 package 解決を行わない
- 生成済み bundle、checksum、生成手順、検証手順を repository 管理する
- Biome を TypeScript runtime source と scripts の formatter / linter gate として導入し、緩い例外設定を作らない
- 既存 TypeScript scripts の `unknown` / `Record<string, unknown>` / Biome ignore を棚卸しし、runtime source へ緩い型境界を持ち込まない
- AST lint を強化し、合意済み階層、生成済み bundle 参照、TypeScript toolchain の runtime 非依存を検査する
- runtime asset 更新と reference score の既存方針を維持し、bundle 生成によって外部 network 依存や CI 上の再生成を増やさない

## Capabilities

### New Capabilities

- `diagram-runtime-bundle-management`: KCF が TypeScript source から Mermaid / Draw.io / ZenUML 別の生成済み JavaScript bundle を管理し、V8 には JavaScript だけを渡す能力
- `runtime-code-quality-gates`: TypeScript runtime source、生成済み bundle、Rust 側 include 境界を Biome と AST lint で検査する能力

### Modified Capabilities

- `runtime-asset-versioning`: upstream vendor asset に加えて、KCF 生成 runtime bundle の checksum と生成再現性を固定する
- `mermaid-zenuml-rendering`: ZenUML 対応 runtime を Mermaid 本体 bundle に埋めず、独立した ZenUML bundle として登録順序を保証する

## Impact

- `crates/katana-canvas-forge/src/markdown/mermaid_renderer/js_runtime/`
- `crates/katana-canvas-forge/src/markdown/drawio_renderer/js_runtime/`
- `crates/katana-canvas-forge/src/markdown/mermaid_renderer/js_runtime_scripts.rs`
- `crates/katana-canvas-forge/src/markdown/drawio_renderer/js_runtime_scripts.rs`
- `crates/katana-canvas-forge/src/markdown/diagram_js_runtime.rs`
- TypeScript runtime source directory and generated bundle directory
- `Justfile` の runtime bundle 生成・検証 recipe
- runtime asset checksum / manifest
- Mermaid / Draw.io / ZenUML render tests、reference compare、runtime asset script tests
- crates.io package include / exclude
