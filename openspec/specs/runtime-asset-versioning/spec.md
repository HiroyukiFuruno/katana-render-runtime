# runtime-asset-versioning Specification

## Purpose
TBD - created by archiving change v0-1-1-runtime-asset-version-pinning. Update Purpose after archive.
## Requirements
### Requirement: Mermaid.js と Draw.io.js の取り込み version を固定しなければならない

システムは、Mermaid.js と Draw.io.js の取り込み version を kdr repository 内で固定しなければならない（MUST）。固定 version は runtime metadata、checksum、reference snapshot の再現性に使われなければならない。TypeScript source から生成される KDR runtime bundle も checksum と生成手順を固定し、upstream vendor asset と混同せずに再現性を検証しなければならない（MUST）。

#### Scenario: Mermaid.js version を固定する

- **WHEN** kdr が Mermaid runtime を初期化する
- **THEN** 固定された Mermaid.js version の asset を読み込む
- **THEN** runtime metadata は Mermaid.js の version と checksum を返す
- **THEN** version が変わった場合は reference snapshot の更新を要求する
- **THEN** KDR 生成 `mermaid-runtime.min.js` の checksum が検証できる

#### Scenario: Draw.io.js version を 30.0.2 に固定する

- **WHEN** kdr が Draw.io runtime を初期化する
- **THEN** 固定された Draw.io.js `30.0.2` の asset を読み込む
- **THEN** runtime metadata は Draw.io.js `30.0.2` と checksum を返す
- **THEN** `vendor/drawio/30.0.2/drawio.min.js` と `drawio.min.js.sha256` が repository 管理されている
- **THEN** Draw.io.js version 更新に伴う reference snapshot が review 可能な差分として残る
- **THEN** KDR 生成 `drawio-runtime.min.js` の checksum が検証できる

#### Scenario: ZenUML runtime bundle を固定する

- **WHEN** kdr が ZenUML 対応 runtime を初期化する
- **THEN** 固定された mermaid-zenuml vendor asset を読み込める
- **THEN** KDR 生成 `zenuml-runtime.min.js` の checksum が検証できる
- **THEN** Mermaid.js / Draw.io.js の upstream version と KDR 生成 bundle の checksum を同じ metadata として扱わない

### Requirement: latest 確認と取り込み更新を just recipe で提供しなければならない

システムは、Mermaid.js / Draw.io.js の latest 確認と指定 version 取り込み更新を just recipe として提供しなければならない（MUST）。

#### Scenario: latest version を確認する

- **WHEN** 開発者が latest check recipe を実行する
- **THEN** Mermaid.js と Draw.io.js の現在固定 version と取得可能な latest version を表示する
- **THEN** repository 内の file を変更しない

#### Scenario: 指定 version を取り込む

- **WHEN** 開発者が update recipe に version を指定して実行する
- **THEN** 対象 runtime asset を `vendor/<runtime>/<version>/` に取り込む
- **THEN** checksum と manifest を更新する
- **THEN** full / representative の reference snapshot を再生成する
- **THEN** local full compare と CI/CD representative compare を実行して score 低下を検知する
- **THEN** score が変わる場合は baseline 差分を review できる
- **THEN** CI/CD の通常 compare 経路では reference snapshot を再生成しない

### Requirement: v0.1.0 transfer の挙動を壊してはならない

システムは、v0.1.1 の runtime asset version 固定によって v0.1.0 transfer の rendering / export / score 挙動を壊してはならない（MUST NOT）。

#### Scenario: v0.1.0 fixture を再検証する

- **WHEN** v0.1.1 の変更後に v0.1.0 の Mermaid / Draw.io fixtures を compare する
- **THEN** local full compare で既存 baseline と score policy を満たす
- **THEN** CI/CD representative compare で代表ケースの score policy を満たす
- **THEN** score 低下がある場合は version 更新差分として report に残す

