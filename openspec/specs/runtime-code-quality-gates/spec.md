# runtime-code-quality-gates Specification

## Purpose
TBD - created by archiving change typescript-diagram-runtime-bundles. Update Purpose after archive.
## Requirements
### Requirement: TypeScript runtime source は Biome の厳格 gate を通らなければならない

システムは、TypeScript runtime source と既存 TypeScript scripts を Biome の formatter / linter gate に含めなければならない（MUST）。Biome の設定は、`any`、暗黙 any、non-null assertion、`@ts-ignore` 相当の抑制、barrel file、default export、未使用 code、危険な global / eval を検出できる厳格設定でなければならない（MUST）。Biome または補助検査は、`unknown` と `Record<string, unknown>` も検出しなければならない（MUST）。

#### Scenario: Biome gate を実行する

- **WHEN** 開発者または CI が TypeScript 品質 gate を実行する
- **THEN** runtime TypeScript source と `scripts/**/*.ts` が Biome の対象になる
- **THEN** generated bundle と vendor asset は formatter / linter の修正対象から除外される
- **THEN** `any`、`unknown`、`Record<string, unknown>`、暗黙 any、non-null assertion、`@ts-ignore` 相当の抑制がある場合は失敗する
- **THEN** Biome rule を弱める ignore / suppression はユーザー確認なしに追加できない

### Requirement: TypeScript compiler gate は runtime source の型安全性を検証しなければならない

システムは、TypeScript runtime source を `strict` 相当の compiler 設定で検査しなければならない（MUST）。`noImplicitAny`、`strictNullChecks`、`noUncheckedIndexedAccess`、`exactOptionalPropertyTypes` 相当の設定を弱めてはならない（MUST NOT）。

#### Scenario: TypeScript type check を実行する

- **WHEN** 開発者または CI が type check recipe を実行する
- **THEN** runtime TypeScript source は strict compiler 設定で検査される
- **THEN** nullable でない値を `?` や `| undefined` で逃がした場合は検査で検出できる
- **THEN** vendor global 境界は明示 interface で表現される

### Requirement: AST lint は合意済み階層を検査しなければならない

システムは、合意済み階層である `shared` / `mermaid` / `drawio` / `zenuml` / `generated` の境界を AST lint または同等の構造検査で守らなければならない（MUST）。

#### Scenario: 階層境界を検査する

- **WHEN** `just ast-lint` 相当の構造検査を実行する
- **THEN** runtime TypeScript source は `diagram_runtime/source/shared` / `diagram_runtime/source/mermaid` / `diagram_runtime/source/drawio` / `diagram_runtime/source/zenuml` の責務別 directory に置かれている
- **THEN** generated bundle は `diagram_runtime/generated` 配下の runtime 別 artifact として置かれている
- **THEN** `shared` は runtime 固有 entrypoint に依存しない
- **THEN** Mermaid source が Draw.io source に直接依存する、または Draw.io source が Mermaid source に直接依存する場合は失敗する

### Requirement: AST lint は Rust 側 include 先を生成済み bundle に限定しなければならない

システムは、Rust 側の `include_str!` が TypeScript source や旧手書き runtime fragment を直接読み込まないことを検査しなければならない（MUST）。V8 に渡す runtime code は生成済み bundle だけでなければならない（MUST）。

#### Scenario: Rust include 境界を検査する

- **WHEN** `just ast-lint` 相当の構造検査を実行する
- **THEN** `js_runtime_scripts.rs` 相当の Rust file は生成済み `*-runtime.min.js` を参照する
- **THEN** TypeScript source を `include_str!` で読み込む場合は失敗する
- **THEN** 旧 runtime fragment の直接 include が残る場合は失敗する

### Requirement: AST lint は生成済み bundle の手編集と同期漏れを検出しなければならない

システムは、生成済み bundle が source から再生成される artifact であることを検査し、手編集または同期漏れを検出できなければならない（MUST）。

#### Scenario: 生成物同期を検査する

- **WHEN** bundle 同期検証と AST lint を実行する
- **THEN** generated bundle に対応する source entrypoint と checksum が存在する
- **THEN** generated bundle だけが変更され、対応する TypeScript source または checksum が変更されない場合は失敗する
- **THEN** checksum 更新だけで source / bundle 差分を隠す場合は失敗する

### Requirement: 既存 TypeScript scripts の緩い型境界を棚卸ししなければならない

システムは、runtime TypeScript source を追加する前に、既存 `scripts/**/*.ts` の `unknown` / `Record<string, unknown>` / Biome suppression を棚卸しし、runtime source へ同じ型境界を持ち込まない移行方針を確定しなければならない（MUST）。

#### Scenario: 既存 scripts を棚卸しする

- **WHEN** TypeScript 品質 gate を導入する
- **THEN** 既存 scripts の `unknown` / `Record<string, unknown>` / suppression comment の一覧を出す
- **THEN** JSON parse など外部入力境界は専用 validator または明示 interface へ移す
- **THEN** runtime source では `unknown` / `Record<string, unknown>` を許可しない
