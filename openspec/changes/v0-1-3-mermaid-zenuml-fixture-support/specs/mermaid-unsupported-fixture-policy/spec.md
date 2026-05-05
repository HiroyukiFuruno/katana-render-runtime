## ADDED Requirements

### Requirement: unsupported Mermaid fixture を明示しなければならない

システムは、公式 runtime で描画できない Mermaid fixture を暗黙に skip してはならない（MUST NOT）。unsupported とする場合は、fixture 名と理由を report に残さなければならない（MUST）。

#### Scenario: ZenUML fixture が runtime で未対応である

- **GIVEN** `28-zen-uml.md` が公式 runtime で `UnknownDiagramError` を返す
- **WHEN** full compare を実行する
- **THEN** compare は null 参照で落ちない
- **THEN** fixture 名と unsupported reason を report に残す
- **THEN** fallback SVG を生成しない

### Requirement: supported fixture は score 対象でなければならない

システムは、supported と判断した Mermaid fixture を score 対象から外してはならない（MUST NOT）。

#### Scenario: ZenUML を supported にする

- **GIVEN** ZenUML が runtime 設定で描画可能である
- **WHEN** full compare を実行する
- **THEN** `28-zen-uml.md` は通常の score 対象になる
- **THEN** unsupported report には入らない
