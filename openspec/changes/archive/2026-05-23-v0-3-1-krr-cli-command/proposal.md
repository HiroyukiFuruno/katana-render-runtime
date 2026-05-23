## Why

`katana-render-runtime` は v0.3.0 で KRR を正本名にしたが、公開 CLI（コマンドライン実行口）はまだ `kdr` のままである。最新 KRR として利用者が実行する入口は `krr xxx` であるべきなので、v0.3.1 で公開表示と実行名を揃える。

## What Changes

- CLI の実行ファイル名（binary）は `krr` を正本にする。
- `krr mermaid ...`、`krr drawio ...`、`krr plantuml ...` を公開コマンドとして提供する。
- CLI package は `katana-render-runtime-cli` を正本として扱い、`cargo install katana-render-runtime-cli` で `krr` が入る形にする。
- PlantUML 向けの公開環境変数（environment variable: 実行時に外から渡す設定名）は `KRR_PLANTUML_*` を正本にし、既存 `KDR_PLANTUML_*` は必要な範囲で互換入力として扱う。
- README、badge、install snippet、help 表示、release docs、OpenSpec の現行仕様から、最新CLIとしての `kdr` 表記を撤去する。
- Justfile の開発用 recipe、prebuilt binary path、CI smoke check を `krr` 前提に更新する。
- CLI parser test と結合テストは `CARGO_BIN_EXE_krr` と `krr ...` の実行を検証する。
- 旧 `kdr` 表記は、互換説明、履歴資料、旧 crate wrapper、内部 linter 名など、残す理由を説明できる箇所に限定する。
- v0.3.1 release として version、package、publish 手順の影響を確認する。
- `katana-diagram-renderer-cli` と `kdr` binary は v0.3.1 では更新・同梱しない。

## Capabilities

### New Capabilities

- `krr-cli-command`: KRR の公開 CLI 名、サブコマンド、互換境界、ドキュメント更新、検証条件を定義する。

### Modified Capabilities

- なし。既存仕様内の `kdr` コマンド例は、この change の tasks で `krr-cli-command` の要件へ追従させる。

## Impact

- `crates/katana-render-runtime-cli/Cargo.toml`
- `crates/katana-render-runtime-cli/src/commands.rs`
- `crates/katana-render-runtime-cli/tests/`
- `Justfile`
- `README.md`
- `docs/release.md`
- `docs/repository-standardization.md`
- `openspec/project.md`
- `openspec/specs/**/*.md`
- release scripts and package metadata
