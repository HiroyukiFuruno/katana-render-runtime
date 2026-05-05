## ADDED Requirements

### Requirement: CSV parse 診断を UI 非依存で返さなければならない

システムは、CSV parse と rendering の診断を KatanA UI に依存しない構造化 diagnostics として返さなければならない（MUST）。

#### Scenario: 文字コードが扱えない

- **WHEN** CSV が UTF-8 として読めない
- **THEN** システムは render error または diagnostic を返す
- **THEN** error には path、byte offset、encoding hint を含める
- **THEN** 文字化けした table を暗黙 fallback として返さない

#### Scenario: delimiter を推定する

- **WHEN** delimiter hint なしで CSV を render する
- **THEN** システムは comma、tab、semicolon などの候補から delimiter を推定する
- **THEN** 推定結果と confidence を metadata に含める
- **THEN** 推定できない場合は明示的な diagnostic を返す

### Requirement: CSV fixture による回帰テストを持たなければならない

システムは、CSV viewer rendering の主要 pattern を fixture で検証しなければならない（MUST）。

#### Scenario: fixture を検証する

- **WHEN** CSV viewer rendering tests を実行する
- **THEN** normal、quoted、multiline、wide、large、encoding fixture を検証する
- **THEN** expected metadata と HTML fragment を比較する
- **THEN** viewer e2e で少なくとも 1 case の実表示を確認する
