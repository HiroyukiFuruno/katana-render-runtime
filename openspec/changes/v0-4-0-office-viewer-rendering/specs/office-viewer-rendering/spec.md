## ADDED Requirements

### Requirement: Word / Excel / PPTX の viewer rendering に限定しなければならない

システムは、Office viewer rendering の対象を Word `.docx`、Excel `.xlsx`、PPTX `.pptx` に限定しなければならない（MUST）。Office 文書の編集、Office format への書き出し、v0.4.x での追加 feature は含めてはならない（MUST NOT）。

#### Scenario: 対応 Office format を rendering する

- **WHEN** 開発者が `.docx`、`.xlsx`、または `.pptx` を Office viewer renderer に渡す
- **THEN** システムは表示用 artifact を生成する
- **THEN** rendering result に artifact path と page / sheet / slide metadata を含める

#### Scenario: non-goal の format を渡す

- **WHEN** 開発者が `.doc`、`.xls`、`.ppt`、macro enabled file、または password protected file を渡す
- **THEN** システムは unsupported input として明示的な error を返す
- **THEN** 暗黙 fallback で空表示や部分表示の成功扱いにしない

### Requirement: Office rendering は KatanA UI state に依存してはならない

システムは、Office viewer rendering の公開 API に KatanA 固有の workspace state、preview state、UI state を含めてはならない（MUST NOT）。KatanA 側は kcf が生成した artifact と metadata を利用する consumer として扱わなければならない（MUST）。

#### Scenario: generic library として呼び出す

- **WHEN** consumer が Office viewer rendering API を呼び出す
- **THEN** 入力は file path、format、rendering option、output directory で表現される
- **THEN** KatanA 固有型を引数に要求しない
- **THEN** result は KatanA 以外の consumer でも解釈できる metadata を返す

#### Scenario: KatanA consumer compatibility を確認する

- **WHEN** release gate を実行する
- **THEN** KatanA 側で必要な artifact path、dimensions、page / sheet / slide metadata、warning、error code が維持されている
- **THEN** 破壊的な API 変更がある場合は release 前に検出される

### Requirement: Office rendering engine は version と checksum を固定しなければならない

システムが外部 process、runtime bundle、または変換 engine を使う場合、その version と checksum を固定しなければならない（MUST）。商用 Office アプリケーションのインストールを必須条件にしてはならない（MUST NOT）。

#### Scenario: rendering engine を更新する

- **WHEN** 開発者が Office rendering engine を更新する
- **THEN** version、checksum、reference snapshot を同じ change で更新する
- **THEN** 更新手順を just recipe または document で再現できる

#### Scenario: checksum が一致しない

- **WHEN** runtime bundle または外部 binary の checksum が期待値と一致しない
- **THEN** rendering を実行せず非0終了する
- **THEN** 失敗理由に expected checksum と actual checksum を含める

### Requirement: Office viewer rendering は reference と viewer e2e で検証できなければならない

システムは、Word / Excel / PPTX の最小 case と代表 case を reference snapshot と viewer e2e で検証できなければならない（MUST）。

#### Scenario: viewer e2e で Office case を開く

- **WHEN** 開発者が Office viewer e2e case を開く
- **THEN** reference artifact と kcf artifact が比較表示される
- **THEN** Word は page、Excel は sheet、PPTX は slide の単位で確認できる

#### Scenario: 自動品質 gate を実行する

- **WHEN** CI または release gate が Office rendering check を実行する
- **THEN** unit test、integration test、viewer e2e smoke、score report、lint、AST lint が実行される
- **THEN** fixture、snapshot、runtime bundle の package 混入を検出できる
