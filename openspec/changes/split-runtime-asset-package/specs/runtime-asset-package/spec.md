## ADDED Requirements

### Requirement: Byte-identical self-contained assets

資産crate SHALL 既存Draw.ioの圧縮JSをバイト単位で保持し、runtime SHALL 従来の展開・キャッシュとchecksum検証を維持する。

#### Scenario: Offline Drawio runtime
- **WHEN** 公開レジストリから取得したruntimeでDraw.ioを描画する
- **THEN** 実行時資産ダウンロードを必要とせず、展開後JSの既存SHA-256と描画回帰を満たす

### Requirement: Bounded publishable packages

全公開crate SHALL 現行10,485,760 bytesの配布容量上限を満たし、公開前に未公開workspace依存を含む実package検証ビルドを通過する。

#### Scenario: Package exceeds existing limit
- **WHEN** いずれかの公開crateが上限を超える
- **THEN** release検証は失敗し、閾値を上げて公開してはならない

#### Scenario: Asset crate is not yet published
- **WHEN** workspaceの資産crateがまだ公開されていない
- **THEN** 公開予定package集合のstagingによる配布検証ビルドとregistry用lockfileを確認する

### Requirement: Dependency-ordered publication

release SHALL assets、runtime、CLIを同一versionで依存順に公開し、それぞれのregistry取得を確認してから次へ進む。

#### Scenario: Retry after partial publication
- **WHEN** 資産crateだけが既に公開されている
- **THEN** 公開済み資産を確認し、runtimeとCLIの公開を再開する

### Requirement: Update synchronization

依存更新 SHALL 最新の資産を一つの圧縮保存先へ生成し、runtime参照・catalog・checksum・配布内容の一致を検証する。

#### Scenario: Updated Drawio version
- **WHEN** 公式Draw.ioの更新を取り込む
- **THEN** 圧縮bytes、展開後checksum、公開package収録を標準ゲートで確認し、古いversionへ恒久固定しない
