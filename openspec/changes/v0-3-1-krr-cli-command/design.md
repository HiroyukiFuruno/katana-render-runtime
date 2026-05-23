## Context

v0.3.0 で repository と primary crate は `katana-render-runtime` に移行済みだが、調査時点では公開 CLI（コマンドライン実行口）と周辺の実行面に旧名 `kdr` が残っている。

主な残存箇所は次の分類である。

- 公開実行名: `crates/katana-diagram-renderer-cli/Cargo.toml` の `[[bin]] name = "kdr"`、`commands.rs` の `#[command(name = "kdr")]`
- 公開テスト: `CARGO_BIN_EXE_kdr`、parser test の `kdr ...`
- 公開ドキュメント: README badge、CLI install、構成説明
- 開発 recipe: `KDR_BIN`、`kdr-build`、`tmp/kdr-*`、compare / render recipe
- OpenSpec 現行仕様: `kdr plantuml ...`、`kdr mermaid compare ...` などの現行コマンド例
- 公開環境変数: `KDR_PLANTUML_JAR`、`KDR_PLANTUML_JVM`、`KDR_PLANTUML_CACHE_DIR`
- 内部・履歴名: `kdr-linter`、archive 済み OpenSpec、過去 release docs、互換 wrapper `katana-diagram-renderer`

## Goals / Non-Goals

**Goals:**

- v0.3.1 の公開 CLI は `krr xxx` で動く。
- 最新 README と release docs は `krr` を正本として案内する。
- CLI package は `katana-render-runtime-cli` を正本候補として扱い、install 後の実行名と package 名のずれを解消する。
- `KRR_PLANTUML_*` を公開環境変数の正本にし、必要なら `KDR_PLANTUML_*` は互換入力として残す。
- `kdr` の残存箇所を「更新対象」「互換として残す対象」「履歴として触らない対象」に分ける。
- v0.3.1 の実装前に、OpenSpec と自動テストで `krr` の受け入れ条件を固定する。

**Non-Goals:**

- 旧 `katana-diagram-renderer` wrapper crate を削除しない。
- archive 済み OpenSpec や過去 release note の履歴表記を機械的に書き換えない。
- `kdr-linter` の内部 crate 名変更は、この change の完了条件にしない。ただし公開面へ出ている場合は別途分類する。
- `kdr` alias を最新 CLI の正本として残さない。互換 alias を追加する場合でも、README と help の正本は `krr` にする。

## Decisions

### 1. v0.3.1 の正本コマンドは `krr`

Cargo の実行ファイル名（binary）と clap の help 表示を `krr` に揃える。`cargo run -p <cli-package> -- --help` は `Usage: krr <COMMAND>` を表示し、parser test も `krr` を入力にする。

旧 `kdr` は v0.3.0 までの挙動として扱う。v0.3.1 で互換 alias を提供するかは実装時に判断できるが、互換 alias を提供しても最新ドキュメントの正本にはしない。

### 2. CLI package 名も KRR へ寄せる

`cargo install katana-diagram-renderer-cli` のまま実行名だけ `krr` にすると、利用者の認識が再びずれる。v0.3.1 の正本 install path は `cargo install katana-render-runtime-cli` を目標にする。

既存 `katana-diagram-renderer-cli` は crates.io 上の過去 package として残る。互換更新を出す場合でも、正本 package と release docs は `katana-render-runtime-cli` に置く。

### 3. 環境変数は `KRR_` を優先する

PlantUML の runtime 解決で使う `KDR_PLANTUML_*` はユーザーが触れる公開面である。v0.3.1 では `KRR_PLANTUML_JAR`、`KRR_PLANTUML_JVM`、`KRR_PLANTUML_CACHE_DIR` を追加し、同名の `KDR_` 変数より優先する。

互換のため `KDR_` 変数をすぐには削除しない。警告文や docs では `KRR_` を先に示し、`KDR_` は旧名互換として説明する。

### 4. 開発用 recipe は正本名に追従する

Justfile は `KRR_BIN` と `krr-build` を正本にし、prebuilt 経路は `target/debug/krr` を使う。`tmp/kdr-*` のような出力ディレクトリ名は実行名ではないが、最新の検証ログとして見えるため、原則 `tmp/krr-*` へ寄せる。

旧 recipe 名を互換 alias として残す場合は、`krr-*` recipe を呼ぶだけにし、正本の説明文は `krr` にする。

### 5. 旧名の残存は分類で管理する

`kdr` を全削除すると、互換 wrapper、過去資料、内部 linter、旧 release の証跡まで破壊する。実装では残存を次のように分類する。

- 更新対象: 最新 CLI、README、release docs、Justfile、現行 OpenSpec specs、テスト
- 互換対象: `katana-diagram-renderer` wrapper、必要な `KDR_PLANTUML_*` fallback、必要なら旧 recipe alias
- 履歴対象: `openspec/changes/archive/**`、過去 release note、v0.1.x / v0.2.x の記録
- 内部対象: `kdr-linter` など、公開 CLI と混同されない内部 crate

## Risks / Trade-offs

- [Risk] CLI package rename により publish 対象が増える → release script と `assert-crates-not-published` の対象を先に更新し、dry-run で package list を確認する。
- [Risk] `KDR_PLANTUML_*` を即削除すると既存利用者が壊れる → v0.3.1 では `KRR_` 優先、`KDR_` fallback として段階移行にする。
- [Risk] `kdr` の機械的一括置換で履歴資料や互換説明を壊す → 置換前に分類表を作り、archive と過去 release note は触らない。
- [Risk] `kdr` alias を残すと「最新も kdr で動く」と誤認される → alias を残す場合も help / README / tests の正本は `krr` だけにする。

## Migration Plan

1. v0.3.1 用 branch 方針を確認する。
2. CLI package 名、binary 名、clap 表示名を `krr` / `katana-render-runtime-cli` へ更新する。
3. `KRR_PLANTUML_*` を追加し、`KDR_PLANTUML_*` fallback より優先する。
4. README、release docs、OpenSpec の現行 specs、Justfile を `krr` 正本へ更新する。
5. CLI unit test と integration test を `krr` 前提へ更新する。
6. `kdr` 残存調査を再実行し、残す箇所には互換・履歴・内部の理由を持たせる。
7. `just check`、release verify、CLI focused tests を通す。

## Open Questions

- `katana-diagram-renderer-cli v0.3.1` を互換 package として publish するか。正本は `katana-render-runtime-cli` だが、旧 package を最新化するかは release 方針として確認する。
- `kdr` alias binary を v0.3.1 に同梱するか。同梱する場合でも、正本 help と docs は `krr` に固定する。
