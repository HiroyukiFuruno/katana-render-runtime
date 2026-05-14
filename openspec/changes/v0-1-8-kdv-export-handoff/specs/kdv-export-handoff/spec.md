## ADDED Requirements

### Requirement: KDRは旧export/debug品質ゲートをKDVへ移譲しなければならない

システムは、旧 export/debug 実装 branch のHTML / PDF / PNG / JPG export、README相対パス解決、file path入力、macOS debug openをKDVへ移譲しなければならない（MUST）。

#### Scenario: 旧release branchを扱う

- **WHEN** 開発者が旧 export/debug 実装 branch の内容を確認する
- **THEN** KDR masterへそのままmergeしない
- **THEN** export/debugに関する論点をKDV側OpenSpecへ移譲する
- **THEN** KDR側には移譲記録だけを残す

### Requirement: KDRは図形描画責務へ限定されなければならない

システムは、KDRの責務をMermaid / Draw.io rendering、runtime asset、reference scoreへ限定しなければならない（MUST）。

#### Scenario: 新しいviewer/export要求が出る

- **WHEN** CSV / PDF / Office viewer またはHTML / PDF / PNG / JPG exportの要求が出る
- **THEN** KDRでは実装しない
- **THEN** KDVのviewer/export pipelineで扱う
- **THEN** KDRにはKDVが呼び出す外部図形描画APIだけを残す
