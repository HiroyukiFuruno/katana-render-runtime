# Draw.io 検証用ファイル（fixture）

このディレクトリは、Draw.io の `.drawio` ファイルパターンを集めた検証用ファイル置き場です。

## 構成

- `basic/`: 最小構成や基本要素を確認するために、このリポジトリ用に作成した単純パターン。
- `official/`: diagrams.net 公式サンプル集から取得した複雑なパターン。
- `representative/`: 継続的統合 / 継続的配信（CI/CD）で使う代表ケース。
- `representative/score-baseline.json`: 現時点で 99 点に届かない代表ケースの既知下限。

## 評価階層

- 疎通確認（smoke check）: `basic/` を使う。描画処理が起動するかを見るだけで、vendor 互換性の保証には使わない。
- 代表ケース評価（representative evaluation）: `representative/` を使う。CI/CD の通常 gate とする。既知差分は `score-baseline.json` に下限を明示する。
- 全量評価（full evaluation）: `basic/` と `official/` 配下の全カテゴリを使う。ローカルの release validation、runtime 更新、score 改善で実行する。

## 件数

- `basic/`: 12 ファイル。
- `official/diagrams/`: 6 ファイル。
- `official/examples/`: 73 ファイル。
- `official/blog/`: 99 ファイル。
- `official/training-diagrams/`: なし。
- `official/templates/`: 420 ファイル。
- `official/libraries/`: 図形ライブラリ（shape library）XML 3 ファイル。
- `official/template-index/`: テンプレート一覧 XML 1 ファイル。
- `representative/`: 20 ファイル。
- `representative/score-baseline.json`: 1 ファイル。

全量評価対象合計: 610 個の `.drawio` ファイル。

> 以下は保持中ファイルと内容が実質重複するため、この repository では fixture 対象外:
> - `official/diagrams/sequence.drawio` → `official/templates/basic/sequence.drawio` と同一内容
> - `official/diagrams/metadata.drawio` → `basic/10-userobject-metadata.drawio` とほぼ同等
> - `official/training-diagrams/`（10 ファイル）→ `basic/` の各ファイルで機能がカバー済み

## 出典

- https://www.drawio.com/blog/example-diagrams-github
- https://www.drawio.com/example-diagrams
- https://github.com/jgraph/drawio-diagrams
- https://www.drawio.com/doc/faq/save-file-formats
- https://www.drawio.com/doc/faq/export-to-xml
- https://www.drawio.com/doc/faq/ai-drawio-generation

## 利用条件

`official/` は `jgraph/drawio-diagrams` の `dev` ブランチから取得しています。

- 取得元 commit: `7b70ed6f96738fed4044a25a314733f8544f076e`
- ライセンス: Apache-2.0
- ライセンス全文: `official/LICENSE.apache-2.0.txt`

図面として開ける元 `.xml` は、検証対象の拡張子を揃えるため `.drawio` として保存しています。
図形ライブラリやテンプレート一覧など、`<mxGraphModel>` を直接含まない補助 XML は `.drawio` にせず、`official/libraries/` と `official/template-index/` に分けています。
