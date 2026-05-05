## ADDED Requirements

### Requirement: CLI は viewer rendering を薄く呼び出せなければならない

システムは、v0.2.0 CSV、v0.3.0 PDF、v0.4.0 Office の viewer rendering を CLI から薄く呼び出せなければならない（MUST）。

#### Scenario: CSV viewer rendering を CLI から実行する

- **WHEN** 利用者が CSV viewer rendering command を実行する
- **THEN** CLI は library の CSV viewer rendering API を呼び出す
- **THEN** artifact path、metadata、diagnostics を output contract に従って出力する

#### Scenario: PDF viewer rendering を CLI から実行する

- **WHEN** 利用者が PDF viewer rendering command を実行する
- **THEN** CLI は library の PDF viewer rendering API を呼び出す
- **THEN** backend error や page metadata を machine readable output で確認できる

#### Scenario: Office viewer rendering を CLI から実行する

- **WHEN** 利用者が Word / Excel / PPTX viewer rendering command を実行する
- **THEN** CLI は library の Office viewer rendering API を呼び出す
- **THEN** page / sheet / slide metadata と warning を output contract に従って出力する
