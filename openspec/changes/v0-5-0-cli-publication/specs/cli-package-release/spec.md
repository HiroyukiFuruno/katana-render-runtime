## ADDED Requirements

### Requirement: CLI package 内容を release gate で固定しなければならない

システムは、CLI 公開前に package metadata、package contents、install 手順を release gate で確認しなければならない（MUST）。

#### Scenario: package list を確認する

- **WHEN** release dry run を実行する
- **THEN** `cargo package --list` で package 内容を確認する
- **THEN** fixture、snapshot、vendor cache、大型 artifact が不要に含まれていないことを確認する
- **THEN** binary name、license、repository、readme が crates.io publish 前提で揃っている

#### Scenario: install 後 smoke を確認する

- **WHEN** packaged CLI を install する
- **THEN** `--version` と `--help` が実行できる
- **THEN** render、export、score、viewer rendering の代表 command が成功する

### Requirement: docs と CLI help は矛盾してはならない

システムは、公開 CLI の docs と `--help` が矛盾しないよう検証しなければならない（MUST）。

#### Scenario: docs を検証する

- **WHEN** docs check を実行する
- **THEN** install command、binary name、public command の記述が CLI help と一致する
- **THEN** KatanA 固有 workflow を前提にした説明だけで公開しない
