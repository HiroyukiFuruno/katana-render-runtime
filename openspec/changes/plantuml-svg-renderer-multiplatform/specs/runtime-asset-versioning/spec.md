## MODIFIED Requirements

### Requirement: Mermaid.js / Draw.io.js / PlantUML JAR の取り込み version を固定しなければならない

システムは、Mermaid.js、Draw.io.js、PlantUML JAR の取り込み version を kdr repository 内で固定しなければならない（MUST）。固定 version は runtime metadata、checksum、reference snapshot の再現性に使われなければならない。TypeScript source から生成される KDR runtime bundle も checksum と生成手順を固定し、upstream vendor asset と混同せずに再現性を検証しなければならない（MUST）。

#### Scenario: Mermaid.js version を固定する

- **WHEN** kdr が Mermaid runtime を初期化する
- **THEN** 固定された Mermaid.js version の asset を読み込む
- **THEN** runtime metadata は Mermaid.js の version と checksum を返す
- **THEN** version が変わった場合は reference snapshot の更新を要求する
- **THEN** KDR 生成 `mermaid-runtime.min.js` の checksum が検証できる

#### Scenario: Draw.io.js version を固定する

- **WHEN** kdr が Draw.io runtime を初期化する
- **THEN** 固定された Draw.io.js の asset を読み込む
- **THEN** runtime metadata は Draw.io.js の version と checksum を返す
- **THEN** Draw.io.js version 更新に伴う reference snapshot が review 可能な差分として残る
- **THEN** KDR 生成 `drawio-runtime.min.js` の checksum が検証できる

#### Scenario: ZenUML runtime bundle を固定する

- **WHEN** kdr が ZenUML 対応 runtime を初期化する
- **THEN** 固定された mermaid-zenuml vendor asset を読み込める
- **THEN** KDR 生成 `zenuml-runtime.min.js` の checksum が検証できる
- **THEN** Mermaid.js / Draw.io.js の upstream version と KDR 生成 bundle の checksum を同じ metadata として扱わない

#### Scenario: PlantUML JAR version を固定する

- **WHEN** kdr が PlantUML runtime を初期化する
- **THEN** 固定された PlantUML JAR を読み込む
- **THEN** runtime metadata は PlantUML の version と checksum を返す
- **THEN** `plantuml.jar` と checksum manifest は review 可能な artifact として管理される
- **THEN** PlantUML JAR version 更新に伴う fixture / reference snapshot 差分が review 可能に残る

### Requirement: latest 確認と取り込み更新を just recipe で提供しなければならない

システムは、Mermaid.js / Draw.io.js / PlantUML JAR の latest 確認と指定 version 取り込み更新を just recipe として提供しなければならない（MUST）。

#### Scenario: latest version を確認する

- **WHEN** 開発者が latest check recipe を実行する
- **THEN** Mermaid.js、Draw.io.js、PlantUML JAR の現在固定 version と取得可能な latest version を表示する
- **THEN** repository 内の file を変更しない

#### Scenario: 指定 version を取り込む

- **WHEN** 開発者が update recipe に version を指定して実行する
- **THEN** 対象 runtime asset を `vendor/<runtime>/<version>/` に取り込む
- **THEN** checksum と manifest を更新する
- **THEN** full / representative の reference snapshot を再生成する
- **THEN** local full compare と CI representative compare を実行して score 低下を検知する
- **THEN** score が変わる場合は baseline 差分を review できる
- **THEN** CI の通常 compare 経路では reference snapshot を再生成しない
