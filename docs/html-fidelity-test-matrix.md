# HTML描画・操作検証の分類基準

ユーザー指定: Chromeを基準に、v0.4.24の先行段階はIssue #109で各画像90点以上、最終段階はIssue #106で各画像99点以上を管理する。旧Issue #110は#106へ統合済み。先行結果を最終合格へ再利用しない。スクロールで見える全領域と、目次全項目・下位項目を網羅する。

## 分類の単位

一つのfixtureには一つの主観点を割り当てる。同じ部品が複数の観点に関係する場合は、検証する性質に応じて別ケースにする。例えばTableの列幅は「表」、Table内の文字の禁則は「文字」、Tableを含む長文の終端到達は「ナビゲーション」で検証する。

| 主観点 | 初期ケース | 境界・状態 | 参照する公式資料 |
| --- | --- | --- | --- |
| 文字 | Typography、見出し、段落、混在inline | 日本語/英語、太字、小数font-size/line-height、禁則、nowrap、長文 | MUI Typography、CSSの使用値・改行規則 |
| 配置・サイズ | Box、Flex、Grid、カード | 狭幅、複数列、余白、intrinsic size、viewport変更 | MUI Layout |
| 表・データ表示 | Table、Badge、複合セル | min-content/percentage列、border-collapse、複数行、文字サイズ混在 | MUI Table、CSS Tables |
| ナビゲーション | 目次、アンカー、sidebar、長文 | 全リンク/下位項目、実click、実最下部、重複付き全Y撮影、固定表示 | USWDS Side navigation、Radix Navigation Menu |
| 入力・状態変化 | Input、Select、Tabs、Accordion | 入力値、選択、開閉、disabled、hover、focus、キーボード | MUI Inputs、Radix操作・アクセシビリティ仕様 |
| 装飾・重なり | Surface、Alert、overlay | 色、角丸、影、gradient、clip、opacity、paint order | MUI Surfaces/Feedback |

## 基準資料と再現性

- 見た目と部品選定: https://mui.com/material-ui/all-components/
- 操作・状態・フォーカス: https://www.radix-ui.com/primitives/docs/overview/accessibility
- 文書・ナビゲーション構成: https://designsystem.digital.gov/
- 規範となる表の計算: https://www.w3.org/TR/css-tables-3/

公式サンプルを参考にしたケースは、出典・対象部品・検証する性質・ローカル入力SHAを記録する。Chrome/KRRは同じローカル入力・viewport・scale・font条件・操作状態で比較する。Reactの出力HTML/CSSの描画と、Reactの実行・状態更新には別のケース識別子と実行証跡を付ける。

## 合格と網羅

各ケースの各viewport、各操作状態、各scroll位置でRMSE/MAEの最小scoreが、先行段階は90以上、最終段階は99以上。平均scoreで低い画像を相殺しない。HTML/binary変更、寸法不一致、資源失敗、scroll不一致、目次リンク漏れ、表示不能は不合格にする。

全文の匿名資料 `tests/fixtures/html/anonymized-requirements.html` は全体受入に使う。原本DOM/CSS/JS・目次階層/順序/リンクを維持し、名称・固有要件・本文・数値・日付は架空内容へ変更したもの。主観点別ケースと全文ケースの両方を比較する。

連続scrollはviewport高の80%以下の間隔で進め、実スクロール値から範囲の抜けを検出する。最上部と実最下部を必須にし、目次の全リンクと下位項目について実クリック後の遷移・該当内容表示・PNGを検証する。固定600/1800は補助点として残す。

分類別HTMLは `tests/fixtures/html/aspects/` に置き、suiteが再帰的に収集する。各ファイル名と主観点はmanifestへ対応させ、追加したケースを未実行のまま合格扱いしない。

## 独立した二つの必須検査

最新ユーザー指示により、代表部品の目視確認から、機能カタログと動的scriptの評価へ範囲を拡大する。

- 見た目: HTML/CSSの部品・レイアウト・文字・装飾を全catalog行へ対応させ、Chrome/KRRの独立PNGを全viewport/状態/全Y位置で比較する。各画像の先行90以上・最終99以上を別々に判定する。
- 動的script: 実HTMLのscriptを両runtimeで実行し、DOM追加/削除/属性/文字/style/classの変化、イベント順序と伝播、input/form、Promise/timer/ready、Observer、scroll/resize、資源追加、再構築後の動作を独立したassertion/state JSONで比較する。操作後のPNG比較も別途必須にする。

Draw.io/Mermaidのように、対象feature・fixture・viewport・状態・Chrome証跡・KRR証跡・PASS/FAIL/未検証をcatalogへ列挙する。対象featureにケースがない、ケースが実行されていない、snapshotしかなく動作結果がない場合は未検証としてgateを失敗させる。code coverageの100%だけではfeature網羅と動的互換の証明にしない。

公式WPTもrendering/layoutのreftestとJavaScript APIのtestharnessを区別している: https://web-platform-tests.org/writing-tests/ 。この分類を検証設計に使用し、公式WPT全件への適合を未実行で主張しない。
