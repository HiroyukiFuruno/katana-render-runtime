## Context

kdr CLI は library の機能を直接実行する入口であり、library より強い責務を持たない。CLI は argument parsing、file I/O、exit code、human readable output、machine readable output を担当し、Mermaid / Draw.io rendering や scoring の中核判断は library に委譲する。

KatanA 側は主に library consumer だが、開発時や release gate では CLI を使って互換性を確認できる必要がある。

KDVへ移譲した viewer/export はKDRの公開CLIへ戻さない。KDR側の公開範囲は、外部図形描画、runtime asset、score、reference 更新に限定する。

## Goals

- 公開 command と output contract を固定する
- install 手順と package metadata を公開前提に整える
- CI と release dry run で publish 前の失敗を検出する
- crates publish dry run を完了条件に含める
- KatanA consumer compatibility を release gate で確認する

## Non-Goals

- CLI に KatanA 固有 workflow を埋め込むこと
- CLI が library 内部実装へ直接依存すること
- GUI viewer を CLI 公開の必須機能にすること
- CSV / PDF / Office viewer rendering を KDR CLI に戻すこと
- HTML / PDF / PNG / JPG export の新規拡張を KDR CLI 公開範囲に含めること
- v0.2.0 で Homebrew、npm、installer など全配布 channel を固定すること

## Public Surface

CLI は binary 名、command、argument、exit code、stdout / stderr、machine readable output を公開 contract として扱う。

公開 command は実装時に既存 CLI と照合して確定するが、少なくとも render、score、reference 更新、version 表示、help 表示を release gate の対象にする。

既存 export command は互換維持対象として棚卸しするが、v0.2.0 で新規拡張しない。KDV実装完了後、KDR側の export 系機能は v0.2.1 で削除する。

## Package And Install

crate metadata は crates.io publish を前提に整える。license、repository、description、readme、keywords、categories、include / exclude を確認し、fixture、snapshot、vendor cache が不要に package へ入らないようにする。

install documentation は `cargo install` を正本とし、別 channel は将来拡張として扱う。

## Release Gate

release 前に次を検証する。

- workspace test と CLI integration test
- lint と AST lint
- `cargo package --list`
- `cargo publish --dry-run`
- CLI help / version / smoke command
- KatanA consumer compatibility

## Compatibility

KatanA 側で利用する公開 API と CLI output は、破壊的変更を release gate で検出する。machine readable output を追加する場合は、field 追加と field 削除を区別し、削除や意味変更は breaking change として扱う。

KDVへ移譲した viewer/export は KDR CLI の互換性fixtureに含めない。KDR側の互換性は、Mermaid / Draw.io rendering、runtime asset、score、reference 更新に限定する。

## Risks

- CLI command が実装都合で増減し、公開 contract が曖昧になる
- package に fixture や snapshot が混入し、install size が増える
- KatanA で必要な output が CLI から読み取りにくくなる
- 既存 export command を公開contractへ含めると、v0.2.1の削除が破壊的変更として重くなる

## Mitigations

- CLI contract test を追加する
- `cargo package --list` を release gate に含める
- KatanA consumer compatibility を固定 fixture で検証する
- export は移譲期間中の互換維持対象として扱い、公開範囲に含めない
