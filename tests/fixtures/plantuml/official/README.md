# PlantUML 公式代表 fixture の抜粋方法

このディレクトリは、PlantUML 公式ドキュメントの全例を網羅するものではない。
各図種につき、公式ページで説明されている主要構文を 1 つの `.puml` にまとめ、KDR が PlantUML の描画責務を担えるかを score 評価するための代表 fixture とする。

比較は、公式サイト上の画像を保存して使う方式ではない。
同じ `.puml` を公式 PlantUML CLI の dark mode で描画した PNG を reference とし、KDR が JVM 埋め込みで描画した SVG を同じ条件で PNG 化して比較する。

## 対応表

| Fixture | 公式ページ | 抜粋した観点 |
| --- | --- | --- |
| `01-sequence.puml` | https://plantuml.com/ja/sequence-diagram | 基本メッセージ、`alt` / `else`、`group`、`loop` |
| `02-use-case.puml` | https://plantuml.com/ja/use-case-diagram | actor、usecase、関連、include / extend 相当の依存線 |
| `03-class.puml` | https://plantuml.com/ja/class-diagram | interface、abstract class、class、enum、note、継承、実装、集約、構成、visibility |
| `04-object.puml` | https://plantuml.com/ja/object-diagram | object 定義、属性、関連、依存 |
| `05-activity.puml` | https://plantuml.com/ja/activity-diagram-beta | `start` / `stop`、`if` / `else`、`repeat`、`backward` |
| `06-component.puml` | https://plantuml.com/ja/component-diagram | component、interface、database、note、依存線 |
| `07-deployment.puml` | https://plantuml.com/ja/deployment-diagram | node、artifact、folder、cloud、database、component、リンク |
| `08-state.puml` | https://plantuml.com/ja/state-diagram | 初期 / 終了 state、遷移、nested state |
| `09-timing.puml` | https://plantuml.com/ja/timing-diagram | robust、concise、時刻指定、message、相対時刻 |

## 更新ルール

- 公式ページから構文観点を増やす場合は、この対応表も同時に更新する。
- 公式画像との見た目比較ではなく、公式 PlantUML CLI 出力との同一入力比較を維持する。
- 公式 theme / dark mode の検証なので、KDR 独自の `skinparam` や SVG 後加工で score を合わせない。
