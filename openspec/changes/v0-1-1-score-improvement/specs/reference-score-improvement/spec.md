## ADDED Requirements

### Requirement: score 改善は既知下限を上げなければならない

システムは、v0.1.0 で移植した reference score の既知未達 case について、現在値を追認するのではなく、修正可能な差分を直して score 下限を上げなければならない（MUST）。

#### Scenario: Draw.io representative baseline を更新する

- **GIVEN** `tests/fixtures/drawio/representative/score-baseline.json` に既知下限がある
- **WHEN** v0.1.1 の score 改善を行う
- **THEN** baseline は改善後の score に合わせて上げる
- **THEN** baseline を下げて合格扱いにしない

#### Scenario: full compare の未達を調査する

- **GIVEN** Draw.io full compare で 99 点未満の case がある
- **WHEN** 開発者が score 改善を実施する
- **THEN** 未達原因を renderer、resource、postprocess、reference 特殊ケースに分類する
- **THEN** 修正しない case は理由と score を report に残す

### Requirement: score 改善は fallback で隠してはならない

システムは、score 比較を通すために fallback SVG、stub PNG、空出力を使ってはならない（MUST NOT）。

#### Scenario: runtime が失敗する

- **GIVEN** Mermaid または Draw.io runtime が失敗する
- **WHEN** compare を実行する
- **THEN** compare は error first で失敗を報告する
- **THEN** fallback 画像で score を作らない
