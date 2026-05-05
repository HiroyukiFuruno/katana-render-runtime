## ADDED Requirements

### Requirement: Office rendering の失敗理由を構造化しなければならない

システムは、Word / Excel / PPTX viewer rendering の失敗理由を構造化 error または warning として返さなければならない（MUST）。

#### Scenario: unsupported Office file を渡す

- **WHEN** `.doc`、`.xls`、`.ppt`、macro enabled file、または password protected file を渡す
- **THEN** システムは unsupported input error を返す
- **THEN** format、reason、recoverability を metadata に含める
- **THEN** 空 artifact を成功扱いで返さない

#### Scenario: rendering engine が利用できない

- **WHEN** Office rendering engine が見つからない、または checksum が一致しない
- **THEN** システムは engine error を返す
- **THEN** expected version、expected checksum、actual checksum を diagnostic に含める
- **THEN** 商用 Office application の install を要求しない

### Requirement: Office rendering は format ごとの表示単位を持たなければならない

システムは、Word / Excel / PPTX それぞれの自然な表示単位を metadata として返さなければならない（MUST）。

#### Scenario: Word を render する

- **WHEN** `.docx` を render する
- **THEN** page または section metadata を返す
- **THEN** text、table、image の warning を artifact metadata に含める

#### Scenario: Excel を render する

- **WHEN** `.xlsx` を render する
- **THEN** sheet name、row range、column range を metadata に含める
- **THEN** 巨大 sheet を暗黙に全量 render しない

#### Scenario: PPTX を render する

- **WHEN** `.pptx` を render する
- **THEN** slide index、slide size、notes support status を metadata に含める
