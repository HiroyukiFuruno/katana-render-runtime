## ADDED Requirements

### Requirement: Office rendering は最小 case と代表 case で検証しなければならない

システムは、Word / Excel / PPTX それぞれに最小 case と代表 case を持ち、reference snapshot と表示確認 case で検証しなければならない（MUST）。

#### Scenario: CI/CD で代表 case を検証する

- **WHEN** CI/CD が Office rendering check を実行する
- **THEN** Word / Excel / PPTX の代表 case を render する
- **THEN** git 管理済み reference snapshot と比較する
- **THEN** reference snapshot を CI/CD 内で再生成しない

#### Scenario: ローカルで full case を検証する

- **WHEN** 開発者が Office full validation を実行する
- **THEN** Word / Excel / PPTX の full fixture を render する
- **THEN** 表示確認 case で reference artifact と kcf artifact を比較表示できる
- **THEN** score 改善対象は v0.4.x の候補として report に残す
