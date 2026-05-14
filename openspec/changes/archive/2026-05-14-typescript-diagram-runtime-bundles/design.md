## Context

現在の KCF は、Mermaid と Draw.io の browser 互換 shim、DOM helper、SVG postprocess、render entrypoint を多数の `.js` ファイルとして保持し、Rust 側の `include_str!` で読み込んだ順に V8 へ渡している。

この方式は外部 process を増やさずに動く一方で、次の問題がある。

- JavaScript file 間の前提が型で表現されない
- helper の共有範囲が Rust 側の script 順序に隠れる
- V8 が多数の script を compile / evaluate する
- ZenUML が Mermaid 系でありながら別 runtime asset として扱うべき境界が見えにくい

V8 は TypeScript runtime ではないため、実行時に TypeScript をそのまま渡す方針は採らない。TypeScript は開発元の言語に限定し、crate に同梱する実行物は JavaScript bundle とする。

## Goals / Non-Goals

**Goals:**

- Runtime adapter の source を TypeScript 化し、型検査で結合の破損を検出する
- Mermaid / Draw.io / ZenUML を別々の bundle として生成する
- 合意済み階層を source / generated / Rust include の全境界で固定する
- Rust 側の V8 実行経路を JavaScript 実行のまま保つ
- 生成済み bundle を repository と crates.io package に含める
- Biome と TypeScript compiler の厳格 gate を追加する
- AST lint を強化し、runtime 階層と generated bundle 境界を検査する
- CI / release で bundle が source から再生成可能か検証する
- vendor asset の version pinning、checksum、reference score 方針を維持する

**Non-Goals:**

- V8 に TypeScript を直接実行させない
- 実行時に Rollup、Bun、Node、Deno、SWC、TypeScript compiler を起動しない
- Mermaid.js / Draw.io.js / mermaid-zenuml の upstream version 更新を同時に行わない
- renderer / exporter の公開 API をこの change で変更しない
- KDVへ移譲済みの export / viewer 方向へ責務を戻さない
- KatanA UI state、preview state、workspace state を runtime bundle に入れない
- Biome / AST lint の例外追加で検査を弱めない

## Decisions

### 1. 合意済み階層を固定する

本 change では、ユーザーと合意した階層を次の形で固定する。

```text
crates/katana-canvas-forge/src/markdown/diagram_runtime/
  source/
    shared/
    mermaid/
    drawio/
    zenuml/
  generated/
    mermaid-runtime.min.js
    drawio-runtime.min.js
    zenuml-runtime.min.js
```

`source/shared` は browser / DOM / SVG helper だけを持ち、`source/mermaid`、`source/drawio`、`source/zenuml` の entrypoint に依存しない。`source/mermaid`、`source/drawio`、`source/zenuml` は相互に直接依存しない。各 runtime 固有の entrypoint は shared helper を取り込んで、対応する `generated/*-runtime.min.js` を生成する。

既存 JS runtime file からの移行先はこの path を正とする。Rust 側は `generated` の bundle だけを `include_str!` で参照する。

### 2. 実行時は JavaScript bundle のみを V8 に渡す

TypeScript source は build-time artifact の入力であり、runtime 入力ではない。Rust 側の `DiagramV8Runtime` は `v8::Script::compile` に JavaScript 文字列だけを渡す。

理由は、V8 の責務が ECMAScript 実行であり、TypeScript の型検査、型除去、module resolution を runtime 境界へ持ち込むと、描画失敗の原因が runtime asset 管理から外れてしまうため。

代替案として V8 実行前に type stripping を行う案があるが、型検査なし、対応構文制限、実行時 toolchain 依存が増えるため採らない。

### 3. Bundle は Mermaid / Draw.io / ZenUML で分離する

出力 bundle は少なくとも次の3つに分ける。

- `mermaid-runtime.min.js`
- `drawio-runtime.min.js`
- `zenuml-runtime.min.js`

Mermaid と ZenUML は近いが、ZenUML は外部 diagram 登録と rasterizable output の制約が強いため、Mermaid 本体 bundle に暗黙で混ぜない。Draw.io は resource / stencil / XML adapter の責務が異なるため独立させる。

共有 helper は TypeScript source 上で `shared` に置く。出力時は各 bundle に必要分を取り込み、V8 実行時に shared module resolver を要求しない。

### 4. 生成済み bundle を repository 管理する

Crate 利用者の build に Rollup などの JavaScript toolchain を要求しない。生成済み bundle は repository にコミットし、`include_str!` で参照する。

Release gate では、source から bundle を再生成した結果がコミット済み bundle と一致することを検証する。これにより、開発時は TypeScript の恩恵を受け、配布時は Rust crate として閉じる。

### 5. Bundle 生成 toolchain は build recipe に閉じる

実装時に Rollup などを導入する場合、`Justfile` に bundle 生成・差分検証・checksum 更新の recipe を用意する。`cargo build` や library runtime は JavaScript toolchain に依存しない。

Toolchain 選定は実装時に確定してよいが、生成物は次を満たす必要がある。

- deterministic に生成できる
- minify 後も stack trace / error origin が追える
- ESM / CommonJS の runtime module 解決を V8 側へ残さない
- crates.io package に生成済み bundle が含まれる

### 6. Biome は TypeScript source の第一 gate にする

現状の repository には root の `biome.json` / `package.json` / `tsconfig.json` がなく、既存 TypeScript scripts は Bun で直接実行されている。さらに既存 scripts には JSON parse 境界で `unknown` / `Record<string, unknown>` を使う箇所と、Biome ignore がある。runtime source を追加するなら、既存 scripts と同じ緩さを runtime adapter に持ち込まないため、Biome gate を先に設計する。

Biome は formatter、import 整理、基本 lint、`any`、non-null assertion、`@ts-ignore` 相当の抑制、barrel file、default export、危険な global / eval の検出に使う。`unknown` / `Record<string, unknown>` は TypeScript compiler だけでは禁止できないため、専用検査または AST lint で検出する。generated bundle と vendor asset は Biome の自動修正対象から外す。

Biome で表現できない型安全性は TypeScript compiler の `strict` 相当設定で補う。`noImplicitAny`、`strictNullChecks`、`noUncheckedIndexedAccess`、`exactOptionalPropertyTypes` を弱めない。

### 7. AST lint は Rust だけでなく runtime 境界も見る

現状の `kcf-linter` は Rust file を対象に、関数長、禁止 method、禁止 type、lazy macro、`allow` 属性、CLI / library 境界を検査している。一方で、TypeScript source、generated bundle、Rust 側 `include_str!` の参照先、bundle checksum の同期は検査対象外である。

この change では AST lint を次の方向で強化する。

- `include_str!` の参照先が generated bundle であることを検査する
- TypeScript source 階層が `shared` / `mermaid` / `drawio` / `zenuml` に分かれていることを検査する
- `shared` から runtime 固有 entrypoint へ依存しないことを検査する
- Mermaid / Draw.io / ZenUML の相互直接依存を禁止する
- TypeScript source の `unknown` / `Record<string, unknown>` / `as any` / suppression comment を検出する
- generated bundle だけの手編集や checksum だけの追認を検出する
- JavaScript toolchain が Rust runtime や `cargo build` に入らないことを検査する

TypeScript の詳細 AST 解析は Biome / TypeScript compiler に任せ、`kcf-linter` は repo 固有の構造契約を担当する。

### 8. Bundle 境界は既存 renderer API の内側に閉じる

`Renderer`、`RenderInput`、`RenderOutput`、CLI command の公開 contract は変えない。変更対象は JS runtime adapter の管理方法と Rust 側 script assembly の内部構造である。

## Risks / Trade-offs

- [Risk] minify により V8 error の原因 file が読みにくくなる → bundle 名と source map / debug bundle 方針を design と test に固定する
- [Risk] bundle 生成差分が大きく review しづらい → source diff、generated bundle checksum、focused runtime tests を併記する
- [Risk] shared helper をまとめすぎて Mermaid / Draw.io / ZenUML の独立性が崩れる → output bundle を分離し、各 runtime の smoke test を独立させる
- [Risk] crates.io package から generated bundle が漏れる → package list gate と runtime smoke test を release-check に入れる
- [Risk] TypeScript 導入で「型を付けるための一時的 any」が増える → 型ルールに従い、DOM shim と vendor 境界は明示型で表現する
- [Risk] Biome ignore や rule downgrade で検査が形骸化する → ignore / suppression 追加は AST lint または review checklist で検出する
- [Risk] AST lint が TypeScript parser まで抱えて複雑化する → TypeScript 文法の詳細は Biome / TypeScript compiler に寄せ、kcf-linter は repo 階層と Rust include 境界に限定する

## Migration Plan

1. 現在の `.js` runtime file を責務ごとに棚卸しし、shared / mermaid / drawio / zenuml の配置を決める
2. Biome / TypeScript compiler 設定を追加し、既存 `scripts/**/*.ts` と runtime source の対象範囲を決める
3. TypeScript source tree と generated bundle tree を作る
4. 既存 JS を TypeScript へ段階的に移し、bundle 生成 recipe を追加する
5. Rust 側の `include_str!` を生成済み bundle 参照へ差し替える
6. AST lint に階層、include 先、generated bundle 同期の repo 固有検査を追加する
7. Mermaid / Draw.io / ZenUML の focused render tests を先に通す
8. reference compare と package include / exclude を確認する
9. 旧 JS source の削除は、生成済み bundle と tests が通った後に別 step として行う

Rollback は、Rust 側の `include_str!` を旧 JS file 群へ戻すことで可能にする。旧 JS source を削除する前に bundle 化の検証を完了させる。

## Open Questions

- 実装時の bundler は Rollup を第一候補にするが、repo の既存 Bun script 運用と合わせて最小 dependency にできるか確認する
- minified bundle だけをコミットするか、debug bundle もコミットするかは、V8 error 調査のしやすさを見て決める
- ZenUML bundle を Mermaid render 時に常に読み込むか、diagram type 判定後に読み込むかは、速度と登録順序の安全性を比較して決める
