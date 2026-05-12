## 1. Runtime Inventory And Boundaries

### Definition of Ready

- [ ] 1.1 現在の `js_runtime/*.js` と Rust 側 `include_str!` 一覧を棚卸ししている
- [ ] 1.2 Mermaid / Draw.io / ZenUML / shared に分ける責務境界を確認している
- [ ] 1.3 V8 へ TypeScript を直接渡さない方針を確認している
- [ ] 1.4 Biome と AST lint の現状検査範囲を確認している

### 目的

既存 runtime script の依存順序と責務を明文化し、TypeScript source と生成済み bundle の配置を決める。

### 書き込み範囲

- `crates/katana-canvas-forge/src/markdown/*_renderer/js_runtime/`
- `crates/katana-canvas-forge/src/markdown/*_renderer/js_runtime_scripts.rs`
- `crates/katana-canvas-forge/src/markdown/diagram_runtime/source/`
- `crates/katana-canvas-forge/src/markdown/diagram_runtime/generated/`
- `openspec/changes/typescript-diagram-runtime-bundles`

### タスク

- [ ] 1.5 Mermaid runtime の既存 JS file と読み込み順序を棚卸しする
- [ ] 1.6 Draw.io runtime の既存 JS file と読み込み順序を棚卸しする
- [ ] 1.7 ZenUML runtime adapter と mermaid-zenuml vendor asset の境界を棚卸しする
- [ ] 1.8 shared helper に移す対象と runtime 固有に残す対象を分類する
- [ ] 1.9 合意済み階層として `crates/katana-canvas-forge/src/markdown/diagram_runtime/source/shared`、`source/mermaid`、`source/drawio`、`source/zenuml`、`generated` の配置を確定する

### Definition of Done

- [ ] Mermaid / Draw.io / ZenUML / shared の境界が design または実装コメントなしで読める構造になっている
- [ ] Rust 側で残す script assembly の責務が最小化されている
- [ ] 実行時 TypeScript 処理が scope 外であることが明確になっている

---

## 2. TypeScript Source, Biome, And Bundle Generation

### 目的

既存 JS runtime adapter を TypeScript source に移し、Mermaid / Draw.io / ZenUML 別の JavaScript bundle を生成する。

### 書き込み範囲

- `crates/katana-canvas-forge/src/markdown/diagram_runtime/source/`
- `crates/katana-canvas-forge/src/markdown/diagram_runtime/generated/`
- `biome.json` or `biome.jsonc`
- `tsconfig.json`
- package manager manifest
- `Justfile`
- bundle generation config
- checksum / manifest file

### タスク

- [ ] 2.1 TypeScript compiler 設定を追加し、`any` / `unknown` / `Record<string, unknown>` に頼らない型境界を定義する
- [ ] 2.2 既存 `scripts/**/*.ts` の `unknown` / `Record<string, unknown>` / Biome ignore を棚卸しする
- [ ] 2.3 JSON parse など外部入力境界を専用 validator または明示 interface で扱う方針へ整理する
- [ ] 2.4 Biome 設定を追加し、runtime TypeScript source と `scripts/**/*.ts` を検査対象にする
- [ ] 2.5 Biome の対象から generated bundle、vendor asset、reference artifact を除外する
- [ ] 2.6 `any`、`unknown`、`Record<string, unknown>`、暗黙 any、non-null assertion、`@ts-ignore` 相当の抑制、barrel file、default export、危険な global / eval を失敗扱いにする
- [ ] 2.7 Mermaid runtime entrypoint を TypeScript source として作る
- [ ] 2.8 Draw.io runtime entrypoint を TypeScript source として作る
- [ ] 2.9 ZenUML runtime entrypoint を TypeScript source として作る
- [ ] 2.10 shared helper を TypeScript source として作る
- [ ] 2.11 `mermaid-runtime.min.js`、`drawio-runtime.min.js`、`zenuml-runtime.min.js` を生成する recipe を追加する
- [ ] 2.12 生成済み bundle の checksum を記録する
- [ ] 2.13 source から再生成した bundle と repository 管理済み bundle の差分検出 recipe を追加する
- [ ] 2.14 Biome gate と TypeScript type check を `just check` または同等の品質 gate に追加する

### Definition of Done

- [ ] 3つの runtime bundle が独立して生成される
- [ ] 生成済み bundle が外部 module resolver を要求しない
- [ ] bundle 同期検証が差分を検出できる
- [ ] Biome と TypeScript type check が runtime source と既存 TS scripts の破損を検出できる

---

## 3. Rust Runtime Integration And AST Lint Strengthening

### 目的

Rust 側の V8 実行経路を JavaScript bundle 読み込みへ差し替え、公開 renderer API は維持する。

### 書き込み範囲

- `crates/katana-canvas-forge/src/markdown/diagram_js_runtime.rs`
- `crates/katana-canvas-forge/src/markdown/mermaid_renderer/js_runtime_scripts.rs`
- `crates/katana-canvas-forge/src/markdown/drawio_renderer/js_runtime_scripts.rs`
- `crates/kcf-linter/src/rules`
- `crates/kcf-linter/tests`
- Mermaid / Draw.io / ZenUML renderer tests

### タスク

- [ ] 3.1 Mermaid runtime script assembly を生成済み bundle 参照へ差し替える
- [ ] 3.2 Draw.io runtime script assembly を生成済み bundle 参照へ差し替える
- [ ] 3.3 ZenUML runtime bundle の読み込み順序と登録順序を固定する
- [ ] 3.4 `DiagramV8Runtime` が TypeScript source を受け取らないことを test で確認する
- [ ] 3.5 公開 `Renderer` API と CLI contract に差分がないことを確認する
- [ ] 3.6 AST lint に `diagram_runtime/source/shared`、`source/mermaid`、`source/drawio`、`source/zenuml`、`diagram_runtime/generated` の階層検査を追加する
- [ ] 3.7 AST lint に Rust 側 `include_str!` の参照先が generated bundle に限定される検査を追加する
- [ ] 3.8 AST lint に shared から runtime 固有 entrypoint へ依存しない検査を追加する
- [ ] 3.9 AST lint に Mermaid / Draw.io / ZenUML の相互直接依存を禁止する検査を追加する
- [ ] 3.10 AST lint に TypeScript source の `unknown` / `Record<string, unknown>` / `as any` / suppression comment を検出する検査を追加する
- [ ] 3.11 AST lint に generated bundle だけの手編集、または checksum だけの追認を検出する検査を追加する

### Definition of Done

- [ ] V8 へ渡す script は生成済み JavaScript のみになっている
- [ ] Mermaid / Draw.io / ZenUML の描画結果が既存期待値を満たしている
- [ ] KatanA UI state、preview state、workspace state が runtime bundle 境界に入っていない
- [ ] AST lint が runtime 階層、include 境界、generated bundle 同期を検出できる

---

## 4. Package And Runtime Asset Gates

### 目的

生成済み bundle、vendor asset、checksum、package 内容を release 前に検証できるようにする。

### 書き込み範囲

- `Justfile`
- `Cargo.toml`
- package include / exclude
- runtime asset checksum / manifest
- release verification scripts

### タスク

- [ ] 4.1 `runtime-asset-check` に生成済み bundle checksum 検証を追加する
- [ ] 4.2 package list gate で生成済み bundle が含まれることを確認する
- [ ] 4.3 `cargo build` が JavaScript toolchain なしで成功することを確認する
- [ ] 4.4 bundle 生成 recipe は開発用 toolchain に閉じ、library runtime から呼ばれないことを確認する
- [ ] 4.5 upstream Mermaid.js / Draw.io.js / mermaid-zenuml の version pinning と generated bundle checksum を分けて report する
- [ ] 4.6 Biome / TypeScript compiler / bundler の依存は開発用 manifest に閉じ、crate runtime dependency と混同しないことを確認する

### Definition of Done

- [ ] Crate 利用者が Rollup、Bun、Node、Deno なしで build できる
- [ ] 生成済み bundle の欠落を package gate で検出できる
- [ ] vendor asset checksum と generated bundle checksum が混同されていない

---

## 5. User Review

> ユーザーから受けた指摘は `[/]` で閉じる。通常の開発タスク `[x]` と混ぜない。

- [ ] 5.1 TypeScript source、生成済み bundle、Rust integration、検証結果をユーザーに提示する
- [ ] 5.2 フィードバックを本 `tasks.md` に追記し、対応済みを `[/]` にする

---

## 6. Final Verification

### 目的

TypeScript bundle 化による描画回帰、score 回帰、package 回帰を検出する。

### タスク

- [ ] 6.1 bundle 生成 recipe を実行する
- [ ] 6.2 bundle 同期検証 recipe を実行する
- [ ] 6.3 Biome gate を実行する
- [ ] 6.4 TypeScript type check を実行する
- [ ] 6.5 `just runtime-asset-check` を実行する
- [ ] 6.6 Mermaid focused render tests を実行する
- [ ] 6.7 Draw.io focused render tests を実行する
- [ ] 6.8 ZenUML focused render tests を実行する
- [ ] 6.9 `just mermaid-compare-ci 99` を実行する
- [ ] 6.10 `just drawio-compare-ci 99` を実行する
- [ ] 6.11 `/lint-and-ast-lint` を実行し、静的検査（lint）と抽象構文木検査（AST lint）の結果を記録する
- [ ] 6.12 `just check` を実行する
- [ ] 6.13 `/self-review` を実行し、差分範囲の設計、テスト、検証の妥当性を確認する
- [ ] 6.14 `npx -y @fission-ai/openspec validate typescript-diagram-runtime-bundles --strict` を実行する
- [ ] 6.15 統合後に `/openspec-archive-change` を実行する

### Definition of Done

- [ ] Mermaid / Draw.io / ZenUML の runtime smoke が成功している
- [ ] representative compare の score が低下していない
- [ ] `just check` と OpenSpec strict validation が成功している
