## Why

v0.1.0 から v0.4.0 までで renderer、viewer rendering、reference 更新、表示確認 case が段階的に揃う。

v0.5.0 では、それらを library 内部の実装に留めず、利用者が install して実行できる CLI として公開する。CLI は library の薄い利用者に限定し、KatanA 固有 state を持たない generic tool として提供する。

## What Changes

- CLI の公開 command、argument、exit code、output contract を固定する
- crate package、binary 名、install 手順、README / docs を公開前提で整える
- CI、release dry run、crates publish dry run を追加する
- v0.1.0 から v0.4.0 までに追加した renderer / exporter / viewer rendering を CLI から薄く呼べるようにする
- KatanA 側 consumer が CLI と library API の互換性を確認できる release gate を追加する
- clean code、test、lint、AST lint、self review を公開前の完了条件に含める

## Non-Goals

- KatanA 専用 CLI にしない
- KatanA UI state、workspace state、preview state を CLI argument にしない
- v0.5.0 で package manager 配布の全 channel を完成させない
- 公開済み command の破壊的変更を軽い修正として扱わない
- GUI viewer を公開 CLI の必須 surface にしない

## Capabilities

### New Capabilities

- `cli-publication`: kcf CLI を install 可能な公開 binary として扱い、docs、package、CI、release dry run、consumer compatibility を検証する

## Impact

- `crates/katana-canvas-forge-cli` — 公開 command、argument、output、exit code
- `crates/katana-canvas-forge` — CLI から利用する公開 API の互換性確認
- `Cargo.toml` / `Cargo.lock` — package metadata と publish 対象
- `README.md` / `docs` — install、usage、API、release 手順
- `.github/workflows` — CI、release dry run、crates publish dry run
- `openspec/changes/v0-5-0-cli-publication/` — 本 change の仕様とタスク
