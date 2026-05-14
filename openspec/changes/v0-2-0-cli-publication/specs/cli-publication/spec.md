## ADDED Requirements

### Requirement: CLI は library の薄い利用者として公開されなければならない

システムは、CLI を `katana-diagram-renderer` library の薄い利用者として公開しなければならない（MUST）。CLI は argument parsing、file I/O、exit code、stdout / stderr を担当し、Mermaid / Draw.io rendering、score、reference 更新の中核判断を再実装してはならない（MUST NOT）。

#### Scenario: CLI から render を実行する

- **WHEN** 利用者が CLI の render command を実行する
- **THEN** CLI は library の公開 API を呼び出す
- **THEN** CLI 独自の rendering 分岐で library と異なる結果を返さない

#### Scenario: CLI help を確認する

- **WHEN** 利用者が `--help` を実行する
- **THEN** 公開 command、argument、output の概要が表示される
- **THEN** KatanA 固有の workspace state、preview state、UI state を要求しない

### Requirement: CLI の公開 contract を test と docs で固定しなければならない

システムは、binary 名、public command、argument、exit code、stdout / stderr、machine readable output を公開 contract として扱い、test と docs で固定しなければならない（MUST）。

#### Scenario: output contract を検証する

- **WHEN** CI が CLI integration test を実行する
- **THEN** `--help`、`--version`、render、score、reference 更新の代表 command が検証される
- **THEN** stdout、stderr、exit code の破壊的変更を検出できる

#### Scenario: machine readable output を変更する

- **WHEN** 開発者が machine readable output の field を削除または意味変更する
- **THEN** compatibility test が失敗する
- **THEN** release 前に breaking change として扱われる

### Requirement: CLI package は install 可能な状態で公開準備されなければならない

システムは、CLI package を crates.io publish 前提で整え、`cargo install` で利用できる install documentation を提供しなければならない（MUST）。

#### Scenario: package metadata を確認する

- **WHEN** release dry run を実行する
- **THEN** package name、binary name、license、repository、description、readme、keywords、categories が確認される
- **THEN** package に不要な fixture、snapshot、vendor cache、大型 artifact が含まれない

#### Scenario: install 手順を確認する

- **WHEN** 利用者が docs の install 手順を読む
- **THEN** `cargo install` による install command が確認できる
- **THEN** install 後の `--version` と smoke command が確認できる

### Requirement: release dry run と crates publish dry run を完了条件に含めなければならない

システムは、CLI 公開前の完了条件に CI、release dry run、`cargo publish --dry-run`、lint、AST lint、self review を含めなければならない（MUST）。

#### Scenario: release dry run を実行する

- **WHEN** release dry run を実行する
- **THEN** workspace test、CLI integration test、package list、CLI smoke、KatanA consumer compatibility が確認される
- **THEN** 失敗した check がある場合は release できない

#### Scenario: crates publish dry run を実行する

- **WHEN** `cargo publish --dry-run` を実行する
- **THEN** publish 前の metadata、include / exclude、dependency、readme の問題が検出される
- **THEN** 成功結果が release note または PR に記録される

### Requirement: KatanA consumer compatibility を generic 境界内で検証しなければならない

システムは、KatanA 側で利用する前提を設計上明記しつつ、CLI と library を KatanA 固有実装にしてはならない（MUST NOT）。consumer compatibility は generic fixture と公開 API / output contract で検証しなければならない（MUST）。

#### Scenario: KatanA consumer compatibility を確認する

- **WHEN** release gate が consumer compatibility check を実行する
- **THEN** render、score、reference 更新の最小 fixture が成功する
- **THEN** KatanA 側で必要な metadata と error code が維持される
- **THEN** UI state を fixture や CLI argument に含めない

#### Scenario: KDVへ移譲した機能を公開CLIに戻さない

- **WHEN** CLI 公開範囲を確認する
- **THEN** CSV / PDF / Office viewer rendering はKDR CLIの公開commandに含まれない
- **THEN** HTML / PDF / PNG / JPG export の新規拡張はKDR CLIの公開範囲に含まれない

#### Scenario: 破壊的変更を検出する

- **WHEN** 公開 API、CLI output、exit code、error code が変更される
- **THEN** compatibility check が変更を検出する
- **THEN** breaking change として release 判断に上げられる
