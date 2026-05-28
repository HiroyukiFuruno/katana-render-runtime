## Implementation Notes

- 作業開始時点の `git status --short --branch` は `master...origin/master` かつ clean。既存 reference score 差分や runtime 差分は混ぜていない。
- OpenSpec 内の旧 `crates/katana-diagram-renderer` 表記は、現行 repo の `crates/katana-render-runtime` へ読み替えて実装した。
- Runtime source の内部依存表現は `package.json` `imports` を正とし、`@shared/...`、`#/shared/...`、`alias` という設計名は使わない。
- Terser は minify / mangle stage として使い、ESM graph、TypeScript transform、package `imports` 解決は Rollup pipeline 側に置く。
- Mermaid / Draw.io の旧 script fragment は通常 script の global 契約を持つため、Rollup IIFE wrapper 内から pre-minify 済み script を global evaluation し、従来の V8 global 挙動を保つ。
- `just drawio-compare-ci 99` は `templates-uml-sequence_1` が 98.79 で失敗する。HEAD の旧 `drawio-runtime.min.js` に一時差し替えても同じ 98.79 だったため、本 change 起因の回帰ではない。

## 1. Readiness And Scope Guard

- [x] 1.1 `git status --short --branch` を確認し、既存の reference score / runtime 差分と本 change の差分を混ぜない方針を記録する
- [x] 1.2 `proposal.md`、`design.md`、`specs/**/*.md` を読み、ESM、`package.json` `imports`、Rollup、Terser、Rust/V8 entry I/F の前提を確認する
- [x] 1.3 現在の `scripts/runtime-bundles/bundle-runtime.ts`、`diagram_runtime/source/**`、`diagram_runtime/generated/**`、Rust 側 `js_runtime_scripts.rs` の実装状態を棚卸しする
- [x] 1.4 `@shared/...` や `#/shared/...` ではなく `#shared/...` 形式の subpath imports を使うこと、`alias` という表現を設計名にしないことを implementation note に残す
- [x] 1.5 Terser 単体では ESM graph / TypeScript transform / package `imports` 解決を担当しないことを実装方針に明記する

## 2. Package Imports And TypeScript Settings

- [x] 2.1 `package.json` に `imports` を追加し、`#shared/*`、`#mermaid/*`、`#drawio/*`、`#zenuml/*` の解決先を `diagram_runtime/source` 配下へ固定する
- [x] 2.2 `package.json` の `type: "module"` を維持し、runtime source と bundle config が ESM 前提で動くことを確認する
- [x] 2.3 `tsconfig.json` の `moduleResolution: "bundler"` と strict 設定が `package.json` `imports` を解決できることを確認する
- [x] 2.4 `resolvePackageJsonImports` が無効化されていないことを確認し、無効化されている場合は有効化する
- [x] 2.5 Runtime source 内の領域またぎ相対 import を `#shared/...` 形式の import へ置き換える
- [x] 2.6 `tsconfig.paths` を正規依存表現として追加しない。必要な補助を追加する場合でも、正本は `package.json` `imports` とする

## 3. Bundle Toolchain

- [x] 3.1 Rollup、`@rollup/plugin-node-resolve`、TypeScript 変換 plugin、Terser plugin/API を dev dependency として追加する
- [x] 3.2 Rollup config を ESM で作成し、Mermaid / Draw.io / ZenUML の3 entry を定義する
- [x] 3.3 `@rollup/plugin-node-resolve` または同等 resolver が package `imports` を解決する設定になっていることを確認する
- [x] 3.4 TypeScript transform は strict type check と責務を分け、bundle 生成時の transform と `tsc --noEmit` の検査を混同しない
- [x] 3.5 出力は `crates/katana-diagram-renderer/src/markdown/diagram_runtime/generated/{mermaid,drawio,zenuml}-runtime.min.js` に固定する
- [x] 3.6 Rollup output は `output.format: "iife"` に固定し、Rust/V8 が通常 script として評価できる形式にする。最終 bundle に `import` / `export` を残さない
- [x] 3.7 Terser minify / mangle を bundle pipeline に入れ、`compress`、`mangle.toplevel`、`mangle.reserved`、`format.comments: false`、source map 無効化、LF 改行を明示する
- [x] 3.8 `katanaRunMermaidRuntime`、`katanaRunDrawioRuntime`、`katanaRunZenumlRuntime` を `globalThis["..."]` で公開し、Terser reserved name でも保護する
- [x] 3.9 `javascript-obfuscator` 等の強い難読化は導入しない。必要になった場合は別 change として扱う
- [x] 3.10 `scripts/runtime-bundles/bundle-runtime.ts` を Rollup JS API の orchestration script として維持し、`--write` / `--check` と checksum manifest 管理を担わせる

## 4. Runtime Source Migration

- [x] 4.1 `source/shared` の helper を ESM export へ移し、runtime 固有 entrypoint へ依存しない構造にする
- [x] 4.2 Mermaid runtime source を ESM import/export へ移し、shared helper は `#shared/...` から import する
- [x] 4.3 Draw.io runtime source を ESM import/export へ移し、shared helper は `#shared/...` から import する
- [x] 4.4 ZenUML runtime source を ESM import/export へ移し、shared helper は `#shared/...` から import する
- [x] 4.5 Mermaid / Draw.io / ZenUML の相互直接 import がないことを確認する
- [x] 4.6 Runtime source の entrypoint を runtime ごとに1つずつ明示し、bundle config の input と一致させる
- [x] 4.7 Vendor global 境界は `@types` 配下の `declare global` と明示 interface で扱い、`any`、`unknown`、`Record<string, unknown>` を使わない
- [x] 4.8 既存の `mermaid_renderer/js_runtime/*.js` 断片を source / generated bundle / vendor asset のどれかへ分類し、重複する handwritten runtime 断片を削除する
- [x] 4.9 既存の `drawio_renderer/js_runtime/*.js` 断片を source / generated bundle / vendor asset のどれかへ分類し、重複する handwritten runtime 断片を削除する

## 5. Rust Runtime Integration

- [x] 5.1 `mermaid_renderer/js_runtime_scripts.rs` が generated `mermaid-runtime.min.js` を読み、Rust 入口 I/F を維持していることを確認する
- [x] 5.2 `drawio_renderer/js_runtime_scripts.rs` が generated `drawio-runtime.min.js` を読み、Rust 入口 I/F を維持していることを確認する
- [x] 5.3 Mermaid 経由 ZenUML path では、Rust 側の diagram type 判定後に generated `zenuml-runtime.min.js` を `mermaid-zenuml.min.js` の後、render script の前へ追加 load する
- [x] 5.4 Mermaid runtime bundle から ZenUML 登録専用 adapter を除去し、登録責務を `zenuml-runtime.min.js` へ移す
- [x] 5.5 Rust 側 render script から `katanaInstallMermaidZenumlRuntimeAdapter()` の直接呼び出しを除去し、`katanaRunMermaidRuntime(requestJson)` だけを呼ぶ形へ寄せる
- [x] 5.6 `zenuml-runtime.min.js` がテスト限定 module ではなく実ロジックで使われることを focused test で確認する
- [x] 5.7 `DiagramV8Runtime` は runtime module resolver を追加せず、生成済み self-contained script の実行に留める
- [x] 5.8 Renderer / CLI の公開 API に差分がないことを確認する

## 6. Quality Gates And AST Lint

- [x] 6.1 Biome 対象に runtime source と bundle config を含め、generated bundle と vendor asset は対象外にする
- [x] 6.2 TypeScript type check が `#shared/*`、`#mermaid/*`、`#drawio/*`、`#zenuml/*` 形式の subpath imports を解決できることを検証する
- [x] 6.3 AST lint に `diagram_runtime/source/**/*.ts` の領域またぎ相対 import 禁止と `#/shared/...` 形式の禁止を追加する
- [x] 6.4 AST lint に `@shared/...` 等の独自 alias 禁止を追加する
- [x] 6.5 AST lint に shared から runtime 固有領域への依存禁止を追加する
- [x] 6.6 AST lint に Mermaid / Draw.io / ZenUML の相互直接 import 禁止を追加する
- [x] 6.7 `just runtime-bundle-check` に、bundle toolchain が ESM graph、package `imports`、TypeScript transform、Rollup `output.format: "iife"` を扱う構成かを確認する検査を追加する
- [x] 6.8 `just runtime-bundle-check` に、Terser 単体構成を禁止し、Terser が minify / mangle stage として使われることを確認する検査を追加する
- [x] 6.9 AST lint または focused test に、`globalThis["katanaRun..."]` entry I/F が保持され、Rust 側 render script が `katanaInstallMermaidZenumlRuntimeAdapter()` を直接呼ばないことの検査を追加する
- [x] 6.10 Generated bundle だけの手編集、checksum だけの追認、minify stage 抜けを検出できるようにする

## 7. Bundle Generation And Checksum

- [x] 7.1 `just runtime-bundle-build` が Rollup pipeline で3つの `*.min.js` を生成するように更新する
- [x] 7.2 `just runtime-bundle-check` が source から再生成した minify / mangle 後の artifact と repository 管理済み artifact を比較するように更新する
- [x] 7.3 `runtime-bundles.sha256` を minify / mangle 後の最終 artifact checksum として更新する
- [x] 7.4 `just runtime-asset-check` が vendor asset checksum と generated bundle checksum を別々に検証することを確認する
- [x] 7.5 `runtime-bundle-package-check` が generated bundle と checksum manifest の package 含有を検査することを確認する

## 8. Focused Tests

- [x] 8.1 ESM `#shared/...` import を含む runtime source が typecheck で解決される test または fixture を追加する
- [x] 8.2 Rollup 後の generated bundle に `import` / `export` が残らないことを検査する test を追加する
- [x] 8.3 Mermaid runtime の focused render test を実行し、entry I/F と描画結果を確認する
- [x] 8.4 Draw.io runtime の focused render test を実行し、entry I/F と描画結果を確認する
- [x] 8.5 ZenUML runtime の focused render test を実行し、production path が `zenuml-runtime.min.js` を使うことを確認する
- [x] 8.6 Minify / mangle 後も V8 error が最低限 bundle 名と entry 名を含むことを確認する

## 9. Verification

- [x] 9.1 `just runtime-bundle-build` を実行する
- [x] 9.2 `just runtime-bundle-check` を実行する
- [x] 9.3 `just biome` を実行する
- [x] 9.4 `just typecheck` を実行する
- [x] 9.5 `just ast-lint` を実行する
- [x] 9.6 `just runtime-asset-check` を実行する
- [x] 9.7 `just runtime-bundle-package-check` を実行する
- [x] 9.8 `just unit-test` または対象 crate の focused test を実行する
- [x] 9.9 `just mermaid-compare-ci 99` を実行する
- [x] 9.10 `just drawio-compare-ci 99` を実行する
- [x] 9.11 `./scripts/openspec validate esm-imports-runtime-bundles --strict` を実行する
- [x] 9.12 `/self-review` を実行し、OpenSpec、実装差分、検証結果の整合を確認する

## 10. Handoff

- [x] 10.1 変更ファイル、挙動差分、検証結果を日本語でまとめる
- [x] 10.2 既存の unrelated dirty files を巻き込んでいないことを `git status --short` で確認する
- [x] 10.3 実装完了後、ユーザー承認を得てから commit に進む
