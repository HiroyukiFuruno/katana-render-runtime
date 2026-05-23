## 1. Scope Guard

- [ ] 1.1 `git status --short --branch` を確認し、既存差分と v0.3.1 CLI rename を混ぜない
- [ ] 1.2 公開 CLI / package metadata / release に触るため、作業 branch 方針をユーザーに確認する
- [ ] 1.3 `proposal.md`、`design.md`、`specs/krr-cli-command/spec.md` を読み、`krr` 正本と旧 `kdr` 互換境界を確認する
- [ ] 1.4 `kdr` / `KDR` / `katana-diagram-renderer-cli` の残存箇所を、更新対象・互換対象・履歴対象・内部対象に分類する

## 2. CLI Package And Binary

- [ ] 2.1 CLI package の正本名を `katana-render-runtime-cli` に変更する
- [ ] 2.2 Cargo binary（実行ファイル名）を `krr` に変更する
- [ ] 2.3 clap help の command name を `krr` に変更する
- [ ] 2.4 `cargo run -p katana-render-runtime-cli -- --help` が `Usage: krr <COMMAND>` を表示することを確認する
- [ ] 2.5 旧 `katana-diagram-renderer-cli v0.3.1` を互換 package として publish するか、release 方針を確認して記録する
- [ ] 2.6 `kdr` alias binary を同梱するか、release 方針を確認して記録する

## 3. Public Environment Variables

- [ ] 3.1 `KRR_PLANTUML_JAR` を `KDR_PLANTUML_JAR` より優先して解決する
- [ ] 3.2 `KRR_PLANTUML_JVM` を `KDR_PLANTUML_JVM` より優先して解決する
- [ ] 3.3 `KRR_PLANTUML_CACHE_DIR` を `KDR_PLANTUML_CACHE_DIR` より優先して解決する
- [ ] 3.4 warning / diagnostics / docs の案内文を `KRR_PLANTUML_*` 正本へ更新し、`KDR_PLANTUML_*` は旧名互換として説明する
- [ ] 3.5 `KRR_` と `KDR_` が両方設定された場合に `KRR_` が優先される regression test を追加する

## 4. Development Recipes And Release Scripts

- [ ] 4.1 Justfile の `KDR_BIN` を `KRR_BIN` 正本へ更新する
- [ ] 4.2 `kdr-build` を `krr-build` 正本へ更新し、必要なら旧 recipe は alias として残す
- [ ] 4.3 render / compare / bench の prebuilt 実行 path を `target/debug/krr` に更新する
- [ ] 4.4 `tmp/kdr-*` の最新検証出力を `tmp/krr-*` へ更新する
- [ ] 4.5 release scripts の package 対象を `katana-render-runtime-cli` に更新する
- [ ] 4.6 crates.io 未公開確認と package dry-run が `katana-render-runtime-cli` を対象にすることを確認する

## 5. Documentation And OpenSpec Alignment

- [ ] 5.1 README の CLI badge を `cli-krr` に更新する
- [ ] 5.2 README の CLI install snippet を `cargo install katana-render-runtime-cli` に更新する
- [ ] 5.3 README の実行例を `krr ...` に更新する
- [ ] 5.4 `docs/release.md` と `docs/repository-standardization.md` の CLI package / command 名を `krr` 正本に更新する
- [ ] 5.5 `openspec/project.md` の CLI 方針を `krr` 正本へ更新する
- [ ] 5.6 `openspec/specs/**/*.md` の現行コマンド例を `krr` へ更新する
- [ ] 5.7 `openspec/changes/archive/**` と過去 release note は履歴対象として扱い、機械的一括置換しない

## 6. Tests

- [ ] 6.1 parser test の入力を `krr mermaid render`、`krr drawio compare`、`krr plantuml render` へ更新する
- [ ] 6.2 integration test を `CARGO_BIN_EXE_krr` に更新する
- [ ] 6.3 `krr --help` が `Usage: krr <COMMAND>` を含む test を追加する
- [ ] 6.4 `kdr` だけで成功することを v0.3.1 の受け入れ条件にしない
- [ ] 6.5 `KRR_PLANTUML_*` 優先と `KDR_PLANTUML_*` fallback の focused test を追加する

## 7. Verification

- [ ] 7.1 `./scripts/openspec validate v0-3-1-krr-cli-command --strict` を実行する
- [ ] 7.2 `just fmt-check` を実行する
- [ ] 7.3 `just lint` を実行する
- [ ] 7.4 `just unit-test` を実行する
- [ ] 7.5 `just ast-lint` を実行する
- [ ] 7.6 `just check` を実行する
- [ ] 7.7 v0.3.1 release verify を `katana-render-runtime-cli` 対象で実行する

## 8. Handoff

- [ ] 8.1 変更ファイル、挙動差分、検証結果を日本語でまとめる
- [ ] 8.2 残した `kdr` / `KDR` 表記を分類ごとに報告する
- [ ] 8.3 `katana-diagram-renderer-cli v0.3.1` と `kdr` alias の扱いについて、実装結果または未決事項を明記する
- [ ] 8.4 ユーザー承認を得るまで commit / push / release に進まない
