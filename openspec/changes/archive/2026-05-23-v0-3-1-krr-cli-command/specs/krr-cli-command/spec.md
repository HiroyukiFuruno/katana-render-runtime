## ADDED Requirements

### Requirement: 公開CLIは krr を正本コマンドにしなければならない

システムは、v0.3.1 の公開 CLI（コマンドライン実行口）の正本コマンドを `krr` にしなければならない（MUST）。最新の help、README、release docs、smoke check は `kdr` を正本コマンドとして案内してはならない（MUST NOT）。

#### Scenario: help が krr を表示する

- **WHEN** 利用者が CLI の help を表示する
- **THEN** help は `Usage: krr <COMMAND>` を表示する
- **THEN** subcommand は `mermaid`、`drawio`、`plantuml` を含む
- **THEN** help は `Usage: kdr <COMMAND>` を最新の実行例として表示しない

#### Scenario: mermaid render を krr で実行する

- **WHEN** 利用者が `krr mermaid render --input in.md --output out.svg` を実行する
- **THEN** command は Mermaid render action として処理される
- **THEN** output file には描画結果が書かれる

#### Scenario: drawio compare を krr で実行する

- **WHEN** 利用者が `krr drawio compare --fixtures tests/fixtures/drawio --min-score 99` を実行する
- **THEN** command は Draw.io compare action として処理される
- **THEN** report は KRR 出力と公式 reference の比較として作成される

#### Scenario: plantuml render を krr で実行する

- **WHEN** 利用者が `krr plantuml render --input in.puml --output out.svg` を実行する
- **THEN** command は PlantUML render action として処理される
- **THEN** PlantUML runtime が利用可能な場合は SVG が出力される

### Requirement: CLI package は KRR 名の install path を提供しなければならない

システムは、v0.3.1 の正本 CLI package として `katana-render-runtime-cli` を提供しなければならない（MUST）。`cargo install katana-render-runtime-cli` で `krr` 実行ファイルが導入されなければならない（MUST）。

#### Scenario: cargo install の案内を見る

- **WHEN** 利用者が README の CLI install 手順を確認する
- **THEN** 手順は `cargo install katana-render-runtime-cli` を示す
- **THEN** 導入後の実行例は `krr ...` を示す
- **THEN** `katana-diagram-renderer-cli` は旧 CLI package として説明される場合を除き、最新の install path として表示されない

#### Scenario: package 内容を確認する

- **WHEN** release verify が CLI package 内容を確認する
- **THEN** package 名は `katana-render-runtime-cli` である
- **THEN** package に含まれる executable binary は `krr` である
- **THEN** package metadata は `katana-render-runtime` repository と docs を指す

### Requirement: PlantUML 環境変数は KRR 名を優先しなければならない

システムは、PlantUML runtime 解決の公開環境変数（environment variable: 実行時に外から渡す設定名）として `KRR_PLANTUML_JAR`、`KRR_PLANTUML_JVM`、`KRR_PLANTUML_CACHE_DIR` を提供しなければならない（MUST）。同じ意味の `KDR_PLANTUML_*` が設定されていても、`KRR_PLANTUML_*` が設定されている場合は `KRR_` 側を優先しなければならない（MUST）。

#### Scenario: KRR_PLANTUML_JAR を使う

- **GIVEN** `KRR_PLANTUML_JAR` が readable な `plantuml.jar` を指している
- **WHEN** PlantUML runtime resolver が JAR を解決する
- **THEN** resolver は `KRR_PLANTUML_JAR` の path を使う
- **THEN** warning text は `KRR_PLANTUML_JAR` を正本の設定名として案内する

#### Scenario: KRR と KDR の両方が設定されている

- **GIVEN** `KRR_PLANTUML_CACHE_DIR` と `KDR_PLANTUML_CACHE_DIR` が両方設定されている
- **WHEN** PlantUML cache directory を解決する
- **THEN** resolver は `KRR_PLANTUML_CACHE_DIR` を使う
- **THEN** `KDR_PLANTUML_CACHE_DIR` は互換 fallback としてだけ扱われる

#### Scenario: 旧 KDR 環境変数だけが設定されている

- **GIVEN** `KRR_PLANTUML_JAR` が未設定で `KDR_PLANTUML_JAR` が設定されている
- **WHEN** PlantUML runtime resolver が JAR を解決する
- **THEN** resolver は互換 fallback として `KDR_PLANTUML_JAR` を使える
- **THEN** docs は新規利用者に `KRR_PLANTUML_JAR` を案内する

### Requirement: 旧 kdr 残存は分類されなければならない

システムは、v0.3.1 実装後に残る `kdr` / `KDR` / `katana-diagram-renderer-cli` 表記を、更新対象、互換対象、履歴対象、内部対象のいずれかに分類しなければならない（MUST）。分類できない旧名表記を最新の公開面に残してはならない（MUST NOT）。

#### Scenario: 最新公開面を検査する

- **WHEN** 実装者が `README.md`、`docs/release.md`、`openspec/project.md`、`openspec/specs/**/*.md`、`Justfile`、CLI tests を検査する
- **THEN** 最新CLIの実行例は `krr` を使う
- **THEN** `kdr` は旧名互換または履歴説明として明示される場合だけ残る

#### Scenario: archive を検査する

- **WHEN** 実装者が `openspec/changes/archive/**` を検査する
- **THEN** 過去 release や過去仕様の `kdr` 表記は履歴対象として扱う
- **THEN** archive を最新仕様へ見せるための機械的一括置換は行わない

#### Scenario: 内部 crate 名を検査する

- **WHEN** 実装者が `kdr-linter` の残存を確認する
- **THEN** 公開CLIと混同されない内部ツール名として残すか、別 change で `krr-linter` へ rename するかを記録する
- **THEN** この判断なしに `kdr-linter` を機械的に置換しない

### Requirement: 自動検証は krr を直接確認しなければならない

システムは、v0.3.1 の CLI 変更を目視や手動実行だけで完了扱いしてはならない（MUST NOT）。unit test、integration test、release verify のいずれかで `krr` 実行名と help 表示を直接検証しなければならない（MUST）。

#### Scenario: parser test を実行する

- **WHEN** CLI parser test を実行する
- **THEN** test input は `krr mermaid render`、`krr drawio compare`、`krr plantuml render` を含む
- **THEN** `kdr` だけで parse できることを成功条件にしない

#### Scenario: integration test を実行する

- **WHEN** CLI integration test を実行する
- **THEN** test は `CARGO_BIN_EXE_krr` を使って実行ファイルを起動する
- **THEN** `krr --help` の output は `Usage: krr <COMMAND>` を含む

#### Scenario: release verify を実行する

- **WHEN** v0.3.1 release verify を実行する
- **THEN** `katana-render-runtime-cli` package の dry-run が通る
- **THEN** `krr` binary が package に含まれることを確認する
- **THEN** `katana-diagram-renderer-cli` を v0.3.1 の正本 package として扱わない
