## Why

HTMLの描画像を利用側が表示・操作するには、両軸の内容範囲、スクロール位置、座標系とframeの対応が必要である。現在はY情報だけが公開され、横方向の入力も無視されるため、画像一致だけでは利用可能な描画契約を満たさない。

## What Changes

- 既存の公開frame形式を維持し、同じgenerationに結び付いた両軸metricsを取得するAPIを追加する。
- root documentのX/Y範囲、offset、overflow方針を描画・入力・hit-test・observerと整合させる。
- resize後のmetrics更新とDOM/JS/資源の再利用を実動作で検証する。
- 利用側の表示完了までの時間を性能条件とし、画像精度とメモリ非劣化も維持する。

## Capabilities

### New Capabilities

- `html-consumer-scroll-surface`: HTML frameと世代を共有する寸法・両軸スクロール・入力・resizeの利用契約。

### Modified Capabilities

なし。既存のframe/input/navigation APIは維持し、追加APIと未処理だった横方向入力で既存契約を補完する。

## Impact

KRRのhtml_browser、html_interactive、html_runtimeと契約回帰が対象。KDV/KatanAではUIのバーと中継への接続が必要だが、別repo編集はIssue先行の境界を守る。既存ライブラリ・layout情報を利用し、汎用テスト基盤を新設しない。リリース全体の一次台帳は`docs/quality-gate-improvement-tasks.md`のR03/R04を維持する。
