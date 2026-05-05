## ADDED Requirements

### Requirement: CSV を viewer artifact に render できなければならない

システムは、CSV 入力を KatanA UI に依存しない viewer artifact に変換できなければならない（MUST）。

#### Scenario: 基本 CSV を render する

- **WHEN** CSV file を viewer rendering に渡す
- **THEN** システムは HTML table artifact を返す
- **THEN** row count、column count、delimiter、header presence を metadata に含める
- **THEN** KatanA UI state を output に含めない

#### Scenario: 引用符と複数行 field を保持する

- **WHEN** quoted field、comma inside field、field 内改行を含む CSV を render する
- **THEN** システムは field 境界を壊さず table cell として出力する
- **THEN** 表示用 HTML は escape される
- **THEN** 値の内容を自動変換しない

### Requirement: CSV viewer rendering は範囲指定を持たなければならない

システムは、巨大 CSV を安全に扱うため、行範囲と列範囲を指定して render できなければならない（MUST）。

#### Scenario: 行列範囲を指定する

- **WHEN** `row_offset`、`row_limit`、`column_offset`、`column_limit` を指定する
- **THEN** 指定範囲の cell だけを artifact に含める
- **THEN** metadata に省略された行数と列数を含める
- **THEN** 全量 DOM 生成を行わない

#### Scenario: 既定範囲で render する

- **WHEN** 範囲指定なしで大きな CSV を render する
- **THEN** システムは既定上限内の行列だけを出力する
- **THEN** truncation reason を diagnostics に含める
