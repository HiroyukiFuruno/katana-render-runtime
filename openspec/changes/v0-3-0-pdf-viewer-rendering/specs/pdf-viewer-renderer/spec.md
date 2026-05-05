## ADDED Requirements

### Requirement: PDF を page artifact に render できなければならない

システムは、既存 PDF を KatanA UI に依存しない page artifact に render できなければならない（MUST）。

#### Scenario: 1 page PDF を render する

- **WHEN** PDF file を viewer rendering に渡す
- **THEN** システムは page artifact を返す
- **THEN** page index、width、height、scale を metadata に含める
- **THEN** KatanA UI state を output に含めない

#### Scenario: multi-page PDF を render する

- **WHEN** page range を指定して PDF を render する
- **THEN** 指定 page だけを artifact として返す
- **THEN** document metadata に total page count を含める
- **THEN** 未指定 page を暗黙に render しない

### Requirement: PDF rendering option を明示できなければならない

システムは、PDF の page range、scale、background、max pages を option として指定できなければならない（MUST）。

#### Scenario: scale を指定する

- **WHEN** scale を指定して PDF page を render する
- **THEN** 出力 artifact は指定 scale に対応した寸法になる
- **THEN** metadata に使用した scale を含める
- **THEN** page size と artifact size の対応が検証できる

#### Scenario: max pages を超える

- **WHEN** PDF の page count が `max_pages` を超える
- **THEN** システムは指定上限までの page を render する
- **THEN** truncation diagnostic を返す
