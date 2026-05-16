## Context

現在の KDR は、TypeScript で runtime bundle 定義を持っているが、runtime source 自体は多数の JavaScript 断片と bundle 定義側の順序に依存している。ファイル間の依存は `import` として表現されておらず、保守者や AI agent は `rg` と bundle 定義の断片順を突き合わせて依存関係を推測する必要がある。

本 change は、既存の `typescript-diagram-runtime-bundles` で作られた source / generated 階層を前提に、runtime source を ESM（ECMAScript Modules）へ移し、`package.json` の `imports` による `#` 始まりの subpath imports を正式な依存表現にする。

この設計で重要なのは、開発時の module 化と runtime 実行を混同しないことである。TypeScript source は ESM と `imports` を使うが、Rust 側の V8 は import resolver ではない。最終生成物は、これまで通り `v8::Script::compile` で通常 script として評価できる自己完結 JavaScript bundle でなければならない。

### 議論で確定した前提

- `imports` は `package.json` に記載し、code 側は `@shared/...` ではなく `#shared/...` のような `#` 始まりで import する
- `alias` という表現は避ける。ここで使うのは Node.js / TypeScript の package `imports` と subpath imports である
- Terser 単体は TypeScript transpile、ESM graph 解決、`package.json` `imports` 解決を担当できないため、bundle tool の主役にはしない
- Rollup または esbuild が bundler 候補だが、入口 I/F 保護、複数 entry、生成物制御の明確さから Rollup を第一候補にする
- Terser は bundle 後の minify / mangle を担当する
- Rust から呼ぶ入口 I/F は維持する。ただし既存の `katanaInstallMermaidZenumlRuntimeAdapter()` 直接呼び出しは production path から外し、ZenUML runtime bundle の load side effect または `katanaRunMermaidRuntime()` 内部責務へ吸収する
- 入口 I/F は `globalThis["katanaRunMermaidRuntime"]` のような文字列 property として固定し、minify / mangle / 難読化で壊さない
- `zenuml-runtime.min.js` は生成物だけでなく、実描画経路で使う対象にする

### 参照した仕様前提

- Node.js packages: `package.json` `imports` は package 内部向け mapping であり、specifier は `#` 始まりで使う
- TypeScript modules reference: `moduleResolution` が `node16` / `nodenext` / `bundler` の場合、`#` 始まりの import は nearest `package.json` の `imports` で解決される
- TypeScript 6.0 release note: `#/` 始まりの subpath imports も扱えるが、本 change では Node.js examples に近く、lint pattern が単純な `#shared/*` 形式を正とする
- `@rollup/plugin-node-resolve`: package `exports` / `imports` entrypoints の解決を扱う

## Goals / Non-Goals

**Goals:**

- Runtime TypeScript source を ESM の `export` / `import` で管理する
- Runtime TypeScript source の内部 import を `package.json` `imports` に定義した `#` subpath imports に統一する
- Mermaid / Draw.io / ZenUML の3 entry を Rollup で独立した bundle にする
- Bundle 後は `import` / `export` / runtime module resolver を残さない
- 出力 `*.min.js` に実際の minify と mangle を適用する
- Rust/V8 が呼ぶ入口 I/F を維持し、難読化後も壊さない
- ZenUML runtime bundle を実描画経路で読み込む
- `just runtime-bundle-build` / `just runtime-bundle-check` / `just check` で同期と品質を検証する
- OpenSpec を読んだ別 agent が、Terser 単体を採らない理由と `imports` の使い方を誤解しない状態にする

**Non-Goals:**

- V8 実行時に ESM resolver、import map、Rollup、Bun、Node、Deno、TypeScript compiler を起動しない
- `@shared/...` や `tsconfig.paths` を runtime source の正規 import 手段にしない
- `javascript-obfuscator` の制御フロー変換や文字列暗号化をこの change の必須要件にしない
- Renderer / CLI の公開 API を変更しない
- Mermaid.js / Draw.io.js / mermaid-zenuml / zenuml-core の upstream version 更新を同時に行わない
- Generated bundle を手編集する運用にしない

## Decisions

### 1. `package.json` `imports` を正とし、`#shared/*` 形式の subpath imports へ統一する

Runtime TypeScript source の import は、次のような `#` 始まりの specifier を使う。

```ts
import { SharedDomFragments } from "#shared/dom_fragments";
import { MermaidRuntimeEntrypoint } from "#mermaid/runtime_entrypoint";
```

`package.json` には次のような `imports` を置く。

```json
{
  "type": "module",
  "imports": {
    "#shared/*": "./crates/katana-diagram-renderer/src/markdown/diagram_runtime/source/shared/*",
    "#mermaid/*": "./crates/katana-diagram-renderer/src/markdown/diagram_runtime/source/mermaid/*",
    "#drawio/*": "./crates/katana-diagram-renderer/src/markdown/diagram_runtime/source/drawio/*",
    "#zenuml/*": "./crates/katana-diagram-renderer/src/markdown/diagram_runtime/source/zenuml/*"
  }
}
```

`@shared/*` や `paths` だけに依存する alias は採らない。`paths` は型検査や editor 補助に寄りやすく、実際の bundle 解決とずれる危険があるためである。必要な場合でも補助に留め、正本は `package.json` `imports` とする。

`#/shared/*` のような slash あり形式も TypeScript では扱えるが、この change では採用しない。`#shared/*` のほうが Node.js の subpath imports 例と近く、`#shared` / `#mermaid` / `#drawio` / `#zenuml` を lint pattern として固定しやすいためである。

### 2. TypeScript 設定は `moduleResolution: "bundler"` を第一候補にする

Runtime source は最終的に Rollup で bundle するため、TypeScript 側は bundler 前提の解決に寄せる。`moduleResolution: "bundler"` は package `imports` / `exports` を扱い、bundle 前提の import 解析と相性がよい。

`moduleResolution: "nodenext"` は Node.js 実行互換を強く見る場合の候補だが、今回の runtime source は Node.js で直接実行しない。実行物は Rollup 後の self-contained JavaScript であり、Rust/V8 は Node.js resolution を行わない。

### 3. Bundle toolchain は Rollup を主役にする

採用する tech stack は次を第一候補にする。

| 層 | 採用 | 役割 |
| --- | --- | --- |
| Package manager / runtime script | Bun | 既存 `bun run` 運用を維持する |
| TypeScript compiler | TypeScript | strict type check を行う |
| Bundler | Rollup | ESM graph を runtime 別に1本化する |
| Package imports resolver | `@rollup/plugin-node-resolve` | `package.json` `imports` / `exports` を解決する |
| TypeScript transform | `@rollup/plugin-typescript` | TS を JS へ変換する |
| Minify / mangle | `@rollup/plugin-terser` または Terser API | 圧縮と名前短縮を行う |
| Bundle orchestration | `scripts/runtime-bundles/bundle-runtime.ts` | Rollup JS API を呼び、write/check と checksum manifest を管理する |
| Formatter / linter | Biome | TS source と scripts の品質 gate |
| Repo-specific lint | `kdr-linter` | import 境界、generated 境界、入口 I/F を検査する |

Terser 単体は採らない。Terser は minify / mangle の道具であり、TypeScript transpile、ESM dependency graph、`package.json` `imports` 解決、複数 entry の bundle 管理を担当しないためである。

esbuild は第二候補である。高速で TypeScript transform / bundle / minify まで扱えるが、KDR では bundle 境界、entrypoint 保護、generated checksum、review しやすい config を優先するため、まず Rollup + Terser に寄せる。

### 4. 出力形式は Rust/V8 が通常 script として実行できる形にする

Rust 側の `DiagramV8Runtime` は `v8::Script::compile` で script を順に評価している。したがって、生成済み bundle は V8 実行時に `import` / `export` を含まない。

Rollup の output は runtime ごとに1 file とし、`output.format: "iife"` を正とする。IIFE 内部の entry module が global side effect として入口を `globalThis` に登録する。`output.name` は Rollup の IIFE 要件を満たすための private namespace とし、Rust 側の呼び出し口として扱わない。

```js
globalThis["katanaRunMermaidRuntime"] = function(request) {
  // minified implementation
};
```

Rust 側の呼び出しは次の既存 I/F を維持する。

```js
katanaRunMermaidRuntime(requestJson)
katanaRunDrawioRuntime()
katanaRunZenumlRuntime(source, isDark)
```

既存実装には `katanaInstallMermaidZenumlRuntimeAdapter()` を Rust 側 render script から直接呼ぶ経路があるが、この change では外部 entry I/F から外す。Mermaid 経由 ZenUML の登録は、`zenuml-runtime.min.js` の load side effect または `katanaRunMermaidRuntime()` 内部の明示処理で行い、Rust 側 render script は `katanaRunMermaidRuntime(requestJson)` だけを呼ぶ形へ寄せる。

Terser の `mangle` で内部名は短縮してよいが、`globalThis["..."]` の property 名と Rust が呼ぶ render script は変えない。補助として Terser `mangle.reserved` に `katanaRunMermaidRuntime`、`katanaRunDrawioRuntime`、`katanaRunZenumlRuntime` を設定し、入口名を二重に保護する。

### 5. `*.min.js` は実際に minify / mangle 済みの artifact にする

現状の `*.min.js` は、主に結合済み artifact である。本 change では、`*.min.js` を次の意味に固定する。

- whitespace / comment は削減されている
- dead code は可能な範囲で削除されている
- 内部識別子は mangle されている
- Rust 入口 I/F と vendor global は保持されている
- checksum は minify / mangle 後の最終 artifact に対して計算される

ここでの難読化は、Terser の mangle による識別子短縮を指す。制御フロー変換や文字列暗号化は、描画差分や V8 error 調査を壊すリスクが高いため、この change の必須 scope から外す。

Terser 設定は deterministic に固定する。少なくとも `compress`、`mangle.toplevel`、`mangle.reserved`、`format.comments: false`、source map 無効化、LF 改行を明示し、checksum が環境差で揺れないようにする。

### 6. ZenUML runtime bundle を実ロジックで使う

`zenuml-runtime.min.js` は generated artifact として存在するだけでは不十分である。ZenUML rendering path は、実描画時に `zenuml-runtime.min.js` を読み、`zenuml.js` / `mermaid-zenuml.min.js` などの固定 vendor asset と組み合わせて使う。

Mermaid 内の `zenuml` diagram を Mermaid renderer 経由で処理する場合、Rust 側は既存の diagram type 判定結果を使い、`zenuml` の場合だけ `zenuml-runtime.min.js` を script assembly に追加する。順序は `mermaid-runtime.min.js`、`mermaid.min.js`、`mermaid-zenuml.min.js`、`zenuml-runtime.min.js`、render script とする。登録や V8 renderer に必要な KDR adapter は `zenuml-runtime.min.js` の責務として扱い、Mermaid 本体 bundle の暗黙副作用に寄せない。

ZenUML vendor asset は vendor 配下の固定 version / checksum 管理を維持する。KDR generated bundle に vendor asset を inline せず、vendor asset checksum と generated bundle checksum は別に扱う。

### 7. Vendor global は明示型で扱う

Repository 全体の型安全ルールに従い、runtime TypeScript source では `any`、`unknown`、`Record<string, unknown>` による逃げ道を作らない。Mermaid、Draw.io、ZenUML など vendor global は `declare global` と明示 interface で表現し、`@types` または runtime source 内の型定義に責務を分ける。

JSON parse や vendor callback などの外部入力境界も、専用 validator または明示 interface で受ける。型制約に準拠できない場合は lint 除外ではなく、実装前にユーザーへ相談する。

### 8. Biome と runtime-bundle-check の責務を分ける

Biome は runtime source、bundle config、既存 `scripts/**/*.ts` の formatter / linter gate とする。Generated bundle、vendor asset、reference artifact は Biome の対象から除外する。

Bundle toolchain の意味的検査は `runtime-bundle-check` に寄せる。Rollup config、`@rollup/plugin-node-resolve`、Terser stage、`output.format: "iife"`、minify stage 抜けの検出は TypeScript / Bun 側で検査する。`kdr-linter` は repo 固有の import 境界、Rust include 境界、entry I/F の文字列集合の検査に集中する。

### 9. AI agent が追いやすい import 境界を lint で固定する

保守性と AI agent の作業性を上げるため、import は次のルールにする。

- `diagram_runtime/source/**/*.ts` の領域またぎ import は `#shared/...`、`#mermaid/...`、`#drawio/...`、`#zenuml/...` のみ
- `../shared/...` のような相対 import で領域をまたがない
- `source/shared` から `source/mermaid` / `source/drawio` / `source/zenuml` へ import しない
- Mermaid / Draw.io / ZenUML は相互に直接 import しない
- public entrypoint は各 runtime の entry file に集約する

このルールにより、AI agent は `from "#shared/...";` を検索するだけで依存元と依存先を追える。bundle 定義の順序から暗黙依存を推測する必要を減らす。

### 10. 旧 JavaScript fragment を段階的に除去する

ESM 化の完了条件は、generated bundle の入力が ESM TypeScript source へ移ることである。旧 `mermaid_renderer/js_runtime/*.js` と `drawio_renderer/js_runtime/*.js` は、対応する ESM source へ移植し、検証後に削除する。ただし、大量削除は既存の reference score 差分と混ぜず、責務ごとの小さい単位で進める。

### 11. OpenSpec 上の理解を実装タスクに落とす

今回の議論で誤解が起きやすい点は、実装タスクにも明示する。

- `imports` は package `imports` であり、`alias` という名前で設計しない
- code 側は `#shared/...` のような `#` 始まりで import する
- Terser 単体では ESM / imports の bundle 要件を満たせない
- Rust/V8 の入口 I/F は変更しない
- 生成物は ESM ではなく self-contained script である
- ZenUML bundle は production path で使う

## Risks / Trade-offs

- [Risk] Rollup config が複雑化する -> config と bundle entry を runtime 別に分け、3 entry の責務を design / tasks に固定する
- [Risk] `package.json` `imports` と TypeScript / Rollup の解決結果がずれる -> `tsc --noEmit` と `runtime-bundle-check` の両方で同じ `#` import を検証する
- [Risk] minify / mangle により V8 error が読みにくくなる -> bundle 名、entrypoint 名、必要なら debug build recipe を用意する
- [Risk] mangle が Rust 入口 I/F を壊す -> `globalThis["..."]` 登録と Terser reserved name で入口を保護する
- [Risk] 強い難読化が描画差分を生む -> この change の難読化は Terser mangle に限定し、強い obfuscator は別 change にする
- [Risk] `#shared/*` が広くなりすぎて shared が肥大化する -> AST lint で shared から runtime 固有領域への逆依存を禁止する
- [Risk] ZenUML path の統合で Mermaid rendering が遅くなる -> Rust 側の diagram type 判定後に `zenuml` の場合だけ追加 load する
- [Risk] 現在の作業ツリーが大きく汚れている -> 実装時は OpenSpec change と runtime source migration を関心ごと別に分け、既存 reference score 差分を混ぜない

## Migration Plan

1. 現在の `diagram_runtime/source` と旧 `js_runtime` の依存を棚卸しする
2. `package.json` に `imports` を追加し、`#shared/*` 形式の命名を固定する
3. `tsconfig.json` を `moduleResolution: "bundler"` 前提に調整し、`resolvePackageJsonImports` が有効なことを検証する
4. Rollup config と runtime entry files を追加する
5. Shared / Mermaid / Draw.io / ZenUML source を ESM export/import へ移す
6. 既存 bundle script を Rollup JS API orchestrator に置き換える
7. Terser minify / mangle を bundle pipeline に入れ、入口 I/F を保護する
8. `mermaid-runtime.min.js` / `drawio-runtime.min.js` / `zenuml-runtime.min.js` を再生成する
9. Rust 側の script assembly を generated bundle のみへ寄せ、ZenUML production path が `zenuml-runtime.min.js` を読むようにする
10. 旧 JS fragment を検証後に削除する
11. AST lint に `#` import 境界、相対 import 禁止、entrypoint 保護を追加し、`runtime-bundle-check` に bundle toolchain と generated 同期の検査を追加する
12. Focused runtime tests、bundle check、typecheck、Biome、ast-lint、runtime asset check、package check を通す

Rollback は、Rollup pipeline 導入前の generated bundle と Rust script assembly に戻すことで行う。旧 JS fragment を削除する前に、生成済み bundle と Rust integration の検証を完了させる。

## Resolved Follow-Up Boundaries

- TypeScript の major version 更新はこの change の必須 scope に含めない。既存 toolchain で `package.json` `imports`、`moduleResolution: "bundler"`、`resolvePackageJsonImports` が成立することを検証対象にする
- Rollup の TypeScript transform は、まず `@rollup/plugin-typescript` を使う。速度や output 制御の理由で esbuild plugin が必要になった場合は、別 change で切り替える
- Debug 用 bundle は repository 管理しない。必要な場合は local recipe として生成し、管理対象は production 用の `*.min.js` と checksum に限定する
