## MODIFIED Requirements

### Requirement: Mermaid.js と Draw.io.js の取り込み version を固定しなければならない

システムは、Mermaid.js と Draw.io.js の取り込み version を kdr repository 内で固定しなければならない（MUST）。固定 version は runtime metadata、checksum、reference snapshot の再現性に使われなければならない。TypeScript source から生成される KDR runtime bundle も checksum と生成手順を固定し、upstream vendor asset と混同せずに再現性を検証しなければならない（MUST）。

#### Scenario: Mermaid.js version を固定する

- **WHEN** kdr が Mermaid runtime を初期化する
- **THEN** 固定された Mermaid.js version の asset を読み込む
- **THEN** runtime metadata は Mermaid.js の version と checksum を返す
- **THEN** version が変わった場合は reference snapshot の更新を要求する
- **THEN** KDR 生成 `mermaid-runtime.min.js` の checksum が検証できる

#### Scenario: Draw.io.js version を固定する

- **WHEN** kdr が Draw.io runtime を初期化する
- **THEN** 固定された Draw.io.js version の asset を読み込む
- **THEN** runtime metadata は Draw.io.js の version と checksum を返す
- **THEN** version が変わった場合は resource manifest と reference snapshot の更新を要求する
- **THEN** KDR 生成 `drawio-runtime.min.js` の checksum が検証できる

#### Scenario: ZenUML runtime bundle を固定する

- **WHEN** kdr が ZenUML 対応 runtime を初期化する
- **THEN** 固定された mermaid-zenuml vendor asset を読み込める
- **THEN** KDR 生成 `zenuml-runtime.min.js` の checksum が検証できる
- **THEN** Mermaid.js / Draw.io.js の upstream version と KDR 生成 bundle の checksum を同じ metadata として扱わない
