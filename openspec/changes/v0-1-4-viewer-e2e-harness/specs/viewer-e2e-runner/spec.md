## ADDED Requirements

### Requirement: 実ウィンドウで reference と kcf 出力を比較表示できなければならない

システムは、実表示 E2E により reference 出力と kcf 出力を実ウィンドウで左右比較表示できなければならない（MUST）。

#### Scenario: case を開く

- **WHEN** 開発者が `just viewer-e2e-open case=<name>` を実行する
- **THEN** 実ウィンドウが開く
- **THEN** 左側に reference 出力が表示される
- **THEN** 右側に kcf 出力が表示される
- **THEN** 上部に case 切替、前へ、次へ、拡大縮小、背景色切替の操作がある
- **THEN** 下部にファイル名、寸法、score report path が表示される

#### Scenario: スクリーンショットを保存する

- **WHEN** 開発者が `just viewer-e2e-screenshot case=<name>` を実行する
- **THEN** viewer e2e は指定 case を表示する
- **THEN** `tmp/viewer-e2e/screenshots/` にスクリーンショットを保存する
- **THEN** 保存先 path を出力する

### Requirement: smoke 実行で最小限の表示品質を確認できなければならない

システムは、`just viewer-e2e-smoke` で実表示 E2E の最小確認を実行できなければならない（MUST）。

#### Scenario: smoke を実行する

- **WHEN** `just viewer-e2e-smoke` を実行する
- **THEN** 最小 Mermaid case を表示確認する
- **THEN** 最小 Draw.io case を表示確認する
- **THEN** 最小 export case を表示確認する
- **THEN** 表示領域が空でないことを確認する
- **THEN** スクリーンショット保存に失敗した場合は非0終了する
