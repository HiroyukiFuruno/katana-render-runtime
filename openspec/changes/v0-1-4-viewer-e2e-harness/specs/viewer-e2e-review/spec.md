## ADDED Requirements

### Requirement: ImageMagick score を実表示 E2E で置き換えてはならない

システムは、実表示 E2E を ImageMagick score の代替として扱ってはならない（MUST NOT）。自動採点の正本は v0.1.0 で移植した ImageMagick compare と baseline policy のままとする。

#### Scenario: score と viewer e2e を併用する

- **WHEN** Mermaid / Draw.io の品質を確認する
- **THEN** score 判定は ImageMagick compare を使う
- **THEN** viewer e2e は実ウィンドウ上の見え方を確認する補助として使う
- **THEN** viewer e2e のスクリーンショットだけで score 合格扱いにしない

### Requirement: 自動検証と手動目視確認の境界を明確にしなければならない

システムは、viewer e2e の自動 smoke と手動目視確認の境界を明確にしなければならない（MUST）。

#### Scenario: 自動 smoke で確認する

- **WHEN** viewer e2e smoke を実行する
- **THEN** process 起動、case 読み込み、artifact 読み込み、非空表示、スクリーンショット保存を確認する
- **THEN** 文字欠けや線の重なりの合否を自動判定しない

#### Scenario: 手動目視確認を行う

- **WHEN** 開発者が viewer e2e を開く
- **THEN** 文字欠け、ラベル切れ、線の重なり、余白、背景色、実ウィンドウでの違和感を確認する
- **THEN** 確認結果を README、report、または PR comment に残せる
