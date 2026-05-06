## ADDED Requirements

### Requirement: PDF backend と document error を構造化して返さなければならない

システムは、PDF rendering backend と document の error を構造化 diagnostic として返さなければならない（MUST）。

#### Scenario: backend が利用できない

- **WHEN** PDF rendering backend が見つからない
- **THEN** システムは backend missing error を返す
- **THEN** install / setup に必要な情報を diagnostic に含める
- **THEN** 空 page artifact を fallback として返さない

#### Scenario: encrypted PDF を render する

- **WHEN** password が必要な PDF を password なしで render する
- **THEN** システムは password required diagnostic を返す
- **THEN** PDF 内容を壊れた document として扱わない

### Requirement: PDF fixture による回帰テストを持たなければならない

システムは、PDF viewer rendering の主要 pattern を fixture で検証しなければならない（MUST）。

#### Scenario: fixture を検証する

- **WHEN** PDF viewer rendering tests を実行する
- **THEN** normal、multi-page、rotated、large、invalid fixture を検証する
- **THEN** page metadata と artifact 生成結果を検証する
- **THEN** 表示確認 case で少なくとも 1 case の実表示を確認する
