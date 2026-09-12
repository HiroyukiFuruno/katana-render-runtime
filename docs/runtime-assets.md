# Runtime asset 管理

## 固定対象

| runtime | version | file | sha256 |
| --- | --- | --- | --- |
| Mermaid.js | 12.0.0 | `crates/katana-render-runtime/vendor/mermaid/12.0.0/mermaid.min.js` | `28fca7ae6ebc7ed7bb63bde63136a74bfef14f296a57e403657eeb8b32836073` |
| Mermaid ZenUML | 1.0.0 | `crates/katana-render-runtime/vendor/mermaid-zenuml/1.0.0/mermaid-zenuml.min.js` | `f75662869d22aac58870e52370fe4c46b89b9710bf4b03df93b3227b5e18e730` |
| Draw.io | 31.4.5 | `crates/katana-render-runtime/vendor/drawio/31.4.5/drawio.min.js` | `ec8b66e744a9e5dbf3870129c297385595f55a548f75b952f209548d49610536` |
| MathJax | 4.1.3 | `crates/katana-render-runtime/vendor/mathjax/4.1.3/tex-svg.js` | `23c036deccc0f2374834a47e4032e452419f3ac027bf17e17c104e2746b19f4c` |
| ZenUML Core | 4.3.0 | `crates/katana-render-runtime/vendor/zenuml-core/4.3.0/zenuml.js` | `7e48bc62d5ef65d2f30747da6dc7da01b7175ff9c637dda066a4b92c2d8bdf4d` |
| PlantUML | 1.2026.8 | `crates/katana-render-runtime/vendor/plantuml/1.2026.8/plantuml.jar.sha256` | `1057dd8b346bed26a48ffebe6054e16fc785dda7c91f37f6c19030a4aab8a942` |

MathJax は公式配布の JavaScript asset を repository 上で checksum 管理し、実行に使う自己完結 bundle は
`crates/katana-render-runtime/src/markdown/diagram_runtime/generated/runtime-bundles.sha256`
で固定する。crates.io package には generated bundle を含め、公式 CDN asset は二重収録しない。

## 参照箇所

- `crates/katana-render-runtime/src/markdown/runtime_assets.rs`
- `crates/katana-render-runtime/src/renderer/runtime.rs`
- `crates/katana-render-runtime/src/renderer/output.rs`
- `Justfile`

既定の描画経路では、上記 asset を一時領域の
`katana-render-runtime/vendor/<runtime>/<version>/` へ展開して読み込む。
`MERMAID_JS` / `MERMAID_ZENUML_JS` / `DRAWIO_JS` / `MATHJAX_JS` / `ZENUML_CORE_JS`
を明示した場合だけ、指定 path を優先する。

## 確認

```bash
just depends-update-all
```

`depends-update-all` は Rust・JavaScript 依存を breaking change を含めて更新し、Mermaid.js、ZenUML、Draw.io、MathJax、PlantUML の各最新リリースを取り込む。必要な基準画像、リソース、生成 bundle を更新し、比較と品質ゲートまで実行する。

TypeScript 7 は現在の `@rollup/plugin-typescript` と型互換ではないため、型検査が通る 6.0.3 に自動的に戻す。他の JavaScript 依存は最新へ更新する。

## Mermaid version 更新時の score 回復 tips

version 更新直後に公式 reference と KRR 描画の score が一時的に乖離すること自体は許容する。ただし、同一の Mermaid asset と入力を使った比較で 99 点未満のまま完了扱いにはしない。公式 reference の変化と KRR 互換層の不足を分け、最終的に `just depends-update-all` の 99 点ゲートを通す。

1. `just depends-update-all` が停止したら、`tmp/runtime-update-logs/mermaid-compare-{en,ja}.log` と `tmp/krr-mermaid-full/{en,ja}/comparison/scores.json` から失点した slug を抽出する。
2. 生成 runtime の JavaScript を変更したら、比較前に必ず `just runtime-bundle-build` と `just krr-build` を実行する。source だけを直して generated bundle を更新しないと、古い補正コードのまま比較される。
3. 最初に SVG の `aria-roledescription`、`viewBox`、`max-width`、主要 group の `transform` を公式と KRR で比較する。version 更新による role 名変更は、既存補正が全て素通りする原因になる。
4. 調査中は既存 recipe の fixture glob を使い、失点した図種だけを再比較する。例えば class 図だけなら次を使う。

   ```bash
   just mermaid-compare-prebuilt tests/fixtures/mermaid/en 99 tmp/krr-mermaid-probe '03-*.md'
   ```

5. 対象図種が 99 点以上へ戻ったら英語・日本語の両方を確認し、最後に `just mermaid-compare-full`、`just mermaid-compare-ci`、`just check` を順に通す。

score を通すために threshold、fixture、比較領域を緩めない。公式 reference の再生成は version 更新時の一度だけにし、補正中は同じ reference に対して KRR 出力を改善する。

## Draw.io version 更新時の score 回復 tips

Draw.io の差分は、まず「入力の解釈」と「export geometry」に分ける。HTML comment や entity、`light-dark()`、web font、画像 preload の差は入力解釈を直す。`viewBox`、content crop、0.5px の stroke 境界、HTML label の overflow は export geometry を直す。

1. `tmp/runtime-update-logs/drawio-compare-*.log` と `tmp/krr-drawio-full/*/comparison/scores.json` から 99 点未満だけを抽出し、寸法差、全体の平行移動、色差、文字だけの差に分類する。
2. `official/*.svg` と KRR の `rendered/*.svg` で、root の `width` / `height` / `viewBox`、先頭の shape 座標、content wrapper の `transform` を比較する。PNG だけを見て座標値を推測しない。
3. `light-dark()` は style と paint attribute の両方を一度だけ解決する。既に dark color へ解決した値を後段で再変換しない。
4. fixture 固有のファイル名や slug では分岐しない。source の page 設定、shape family、安定した構造上の特徴で対象を判定し、同じ構造の入力へ再利用できる補正にする。
5. 調査中は失点 fixture の `.drawio` と `official/*.png` だけを一時 directory へ symlink し、既存の `drawio-compare-prebuilt` で短く反復する。恒久的な個別 recipe は追加しない。
6. source JavaScript を変更したら `just runtime-bundle-build`、`just krr-build` の順で反映してから再比較する。対象 fixture が通った後に同じ template family、最後に `just drawio-compare-full 99` を通す。
7. 内容がほぼ同一でも root canvas が 1px 違うと、PNG 比較時の全体リサイズで score が大きく落ちる。まず SVG と PNG の寸法を確認し、必要なら透明領域を trim した内容同士も比べて、描画差と canvas 差を切り分ける。
8. page margin は、上下対称の暗黙余白、source 上端を維持した下端余白、実描画が page 端へ到達する場合を分ける。単一の固定 offset ではなく、source paint bounds と実描画 bounds の関係から crop と edge padding を決める。

0.5px の補正は score 用の画像加工ではなく、Draw.io の browser export が stroke を含めて決める原点を KRR の軽量 DOM で再現するためにのみ使う。最新 runtime が同じ原点を直接出力するようになった場合は補正を残さず、回帰テストと full compare を通して削除する。
