## ADDED Requirements

### Requirement: Mermaid.js と Draw.io.js の取り込み version を固定しなければならない

システムは、Mermaid.js と Draw.io.js の取り込み version を kcf repository 内で固定しなければならない（MUST）。固定 version は runtime metadata、checksum、reference snapshot の再現性に使われなければならない。

#### Scenario: Mermaid.js version を固定する

- **WHEN** kcf が Mermaid runtime を初期化する
- **THEN** 固定された Mermaid.js version の asset を読み込む
- **THEN** runtime metadata は Mermaid.js の version と checksum を返す
- **THEN** version が変わった場合は reference snapshot の更新を要求する

#### Scenario: Draw.io.js version を固定する

- **WHEN** kcf が Draw.io runtime を初期化する
- **THEN** 固定された Draw.io.js version の asset を読み込む
- **THEN** runtime metadata は Draw.io.js の version と checksum を返す
- **THEN** version が変わった場合は resource manifest と reference snapshot の更新を要求する

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
- **THEN** reference snapshot を再生成する
- **THEN** compare を実行して score 低下を検知する

### Requirement: v0.1.0 transfer の挙動を壊してはならない

システムは、v0.1.1 の runtime asset version 固定によって v0.1.0 transfer の rendering / export / score 挙動を壊してはならない（MUST NOT）。

#### Scenario: v0.1.0 fixture を再検証する

- **WHEN** v0.1.1 の変更後に v0.1.0 の Mermaid / Draw.io fixtures を compare する
- **THEN** 既存 baseline と score policy を満たす
- **THEN** score 低下がある場合は version 更新差分として report に残す
