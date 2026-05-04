## Why

v0.1.0 では Mermaid 描画に Rust 管理 JS（Node.js ランタイム相当）を利用しているが、Node.js や Java（PlantUML）への外部プロセス依存はビルド・配布・CI の複雑性を増やす。これらを Rust ネイティブの実装に差し替えることで、kcf を単一バイナリで完結させる。

## What Changes

### Mermaid backend

- Node.js 外部プロセス依存を除去し、Rust ネイティブの Mermaid 描画 backend に切り替える
- 候補: `merman` / Rust 管理 JS（`deno_core` / `v8` ベース）/ `selkie-rs` から評価・選定する
- Mermaid.js 公式互換を維持しつつ、外部プロセスなしで同等の SVG 出力を得る
- 公式比較画像・採点評価で v0.1.0 との出力互換を検証する

### PlantUML backend

- Java 外部プロセス依存を除去し、`plantuml-little` 等の Rust ネイティブ実装に切り替える
- v0.1.0 で対応していない場合は v0.2.0 で新規追加する

### Draw.io export backend

- Draw.io SVG / PNG / PDF export を外部プロセスなしで実現する

## Capabilities

### Modified Capabilities

- `renderer-runtime-interface`: Node.js / Java 依存なしでの Mermaid / PlantUML / Draw.io 描画

### New Capabilities

- `plantuml-backend`: Rust ネイティブ PlantUML 描画（v0.1.0 で未対応の場合）

## Impact

- `crates/katana-canvas-forge/src/mermaid/` — backend 差し替え
- `crates/katana-canvas-forge/src/plantuml/` — 新規追加（または v0.1.0 から拡張）
- `crates/katana-canvas-forge/src/drawio/` — export backend 差し替え
- Node.js / Java の外部プロセス依存が `Cargo.toml` および CI から除去される
