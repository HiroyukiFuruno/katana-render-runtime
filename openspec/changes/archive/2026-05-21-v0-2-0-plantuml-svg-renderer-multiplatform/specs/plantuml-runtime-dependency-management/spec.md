## ADDED Requirements

### Requirement: PlantUML runtime dependency を複数 OS で解決しなければならない

システムは、macOS / Linux / Windows で `libjvm` と `plantuml.jar` を解決できなければならない（MUST）。解決できない場合、どの path と環境変数を確認したかを警告（warning）に含め、ログへ出し、raw code block を返して早期 return しなければならない（MUST）。

#### Scenario: libjvm path を明示指定で解決する

- **GIVEN** `KDR_PLANTUML_JVM` が設定されている
- **WHEN** PlantUML renderer が `libjvm` を解決する
- **THEN** `KDR_PLANTUML_JVM` の path を最優先で使う
- **THEN** 存在しない場合は警告（warning）付き raw code block を返す

#### Scenario: JAVA_HOME から libjvm を解決する

- **GIVEN** `KDR_PLANTUML_JVM` が未設定で `JAVA_HOME` が設定されている
- **WHEN** PlantUML renderer が `libjvm` を解決する
- **THEN** macOS / Linux では `$JAVA_HOME/lib/server/libjvm.*` 相当を候補にする
- **THEN** Windows では `%JAVA_HOME%\\bin\\server\\jvm.dll` 相当を候補にする

#### Scenario: libjvm を認識できない

- **GIVEN** 明示 path が未設定である
- **WHEN** PlantUML renderer が `libjvm` を解決できない
- **THEN** システムは警告（warning）に解決候補と install hint を含める
- **THEN** システムは警告（warning）をログに出す
- **THEN** システムは raw code block を返す
- **THEN** viewer / export は停止しない

### Requirement: PlantUML runtime fallback warning を CLI と公開 API で共通化しなければならない

システムは、PlantUML runtime が利用できないときの警告（warning）を CLI と公開 API で同じ情報から生成しなければならない（MUST）。警告は `plantuml-runtime-unavailable` を識別でき、原因、確認した環境変数、確認した path、利用者が次に行う install / env 設定を含まなければならない（MUST）。

#### Scenario: 公開 API が runtime fallback warning を返す

- **GIVEN** `libjvm` または `plantuml.jar` が認識できない環境
- **WHEN** consumer が `Renderer::render(&RenderInput)` を呼ぶ
- **THEN** 公開 API は raw code block を含む正常な `RenderOutput` を返す
- **THEN** 公開 API は `RenderOutput.diagnostics` に `plantuml-runtime-unavailable` warning を含める
- **THEN** 公開 API は同じ warning をロガーへ出す
- **THEN** warning message は「何が起きたか」と「何を設定すべきか」を読める文にする

### Requirement: JVM 埋め込みを初期実装の第一候補にしなければならない

システムは、Rust process 内で JVM（Java Virtual Machine: Java 仮想マシン）を起動し、PlantUML の SVG 生成 API を呼ぶ経路を初期実装の第一候補にしなければならない（MUST）。子 process 実行は、JVM 埋め込みが multi platform 条件を満たせない場合だけ明示的な代替案として扱わなければならない（MUST）。

#### Scenario: 埋め込み JVM から PlantUML SVG を生成する

- **GIVEN** `libjvm` と `plantuml.jar` が解決できる
- **WHEN** PlantUML renderer が sequence diagram source を処理する
- **THEN** renderer は `plantuml.jar` を classpath に入れて JVM を起動する
- **THEN** renderer は `net.sourceforge.plantuml.SourceStringReader` 相当の API から SVG を取得する
- **THEN** SVG は `<svg`、`viewBox`、source 内の主要 label を含む
- **THEN** 端末窓の制御を必要としない

#### Scenario: JVM の lifetime を renderer 契約に収める

- **GIVEN** 同一 process 内で PlantUML renderer が複数回呼ばれる
- **WHEN** JVM が既に初期化済みである
- **THEN** renderer は JVM を再初期化しない
- **THEN** 異なる `plantuml.jar` への切り替え要求は診断可能な error または warning として扱う
- **THEN** Java 例外、timeout、thread safety の制約を `RenderOutput.diagnostics` または `RenderError` に mapping する

### Requirement: plantuml.jar を固定 URL / checksum の cache runtime asset として解決しなければならない

システムは、`plantuml.jar` を固定 version、download URL、checksum manifest で管理しなければならない（MUST）。crate package には JAR 本体を含めず、checksum manifest を含めなければならない（MUST）。既定では OS 別の保存領域（cache）に JAR を初回 download し、checksum 検証後に JVM classpath へ渡さなければならない（MUST）。

#### Scenario: 明示 jar path を解決する

- **GIVEN** `KDR_PLANTUML_JAR` または既存互換の `PLANTUML_JAR` が設定されている
- **WHEN** PlantUML renderer が JAR を解決する
- **THEN** 明示 path を優先する
- **THEN** 明示 path は利用者管理の JAR として存在確認し、checksum を検証する
- **THEN** path が存在しない場合は警告（warning）付き raw code block を返す

#### Scenario: cache jar path を解決する

- **GIVEN** 明示 jar path が未設定である
- **WHEN** PlantUML renderer が JAR を解決する
- **THEN** KDR は OS 別の保存領域（cache）の固定 path を確認する
- **THEN** 保存領域（cache）に JAR が存在する場合は checksum が manifest と一致することを確認する
- **THEN** 保存領域（cache）に JAR が存在しない場合は固定 URL から download する
- **THEN** download した JAR は checksum を検証してから保存領域（cache）へ配置する
- **THEN** download または checksum 検証に失敗した場合は警告（warning）付き raw code block を返す
- **THEN** 警告（warning）は network 接続、書き込み可能な `KDR_PLANTUML_CACHE_DIR` または API の `plantuml_cache_dir`、または `KDR_PLANTUML_JAR` 設定が必要であることを示す

#### Scenario: API が cache directory を上書きする

- **GIVEN** 公開 API の `RenderInput.config.vendor_config.plantuml_cache_dir` または `plantumlCacheDir` が設定されている
- **WHEN** PlantUML renderer が JAR を解決する
- **THEN** API 指定の保存領域（cache directory）を `KDR_PLANTUML_CACHE_DIR` と OS 既定値より優先する
- **THEN** 保存領域（cache directory）だけの違いでは `cache_fingerprint` を変えない

#### Scenario: CLI が cache directory を上書きする

- **GIVEN** `kdr plantuml render --cache-dir <path>` が指定されている
- **WHEN** CLI が `RenderInput` を作成する
- **THEN** CLI は API と同じ `plantuml_cache_dir` に変換する
- **THEN** `--runtime` と `--cache-dir` が同時に指定された場合は曖昧な解決として拒否する

#### Scenario: user local jar path を解決する

- **GIVEN** 保存領域（cache）内 JAR が存在せず、network からも取得できない
- **WHEN** PlantUML renderer が JAR を解決する
- **THEN** raw code fallback と warning で継続する
- **THEN** legacy KatanA の `~/.local/katana/plantuml.jar` は既定候補にしない

### Requirement: 子 process 実行を採る場合は端末窓を出さずに実行しなければならない

システムは、JVM 埋め込みを採らず PlantUML process を使う場合、OS ごとの shell wrapper ではなく、KDR の process facade から起動しなければならない（MUST）。Windows では端末窓を表示してはならない（MUST NOT）。

#### Scenario: Windows で PlantUML を実行する

- **GIVEN** Windows 上で PlantUML renderer を実行する
- **WHEN** Java process を起動する
- **THEN** process facade は `CREATE_NO_WINDOW` 相当の設定を使う
- **THEN** `cmd` / PowerShell wrapper を経由しない
- **THEN** `javaw.exe` ではなく stdout / stderr pipe が使える `java.exe` を使う

#### Scenario: macOS / Linux で PlantUML を実行する

- **GIVEN** macOS または Linux 上で PlantUML renderer を実行する
- **WHEN** Java process を起動する
- **THEN** `-Djava.awt.headless=true`、`-jar`、`-pipe`、`-tsvg` を明示する
- **THEN** source は stdin で渡し、SVG は stdout から受け取る

### Requirement: CI は複数 OS の PlantUML smoke check を実行しなければならない

システムは、macOS / Ubuntu / Windows の CI で PlantUML の最小 smoke check を実行しなければならない（MUST）。少なくとも Java 解決、JAR 解決、Graphviz `dot` が必要な図の前提整備、sequence fixture の SVG 生成を確認しなければならない（MUST）。

#### Scenario: CI matrix で PlantUML smoke check を実行する

- **WHEN** GitHub Actions が macOS / Ubuntu / Windows job を実行する
- **THEN** `libjvm` が利用可能か確認する
- **THEN** Graphviz `dot` が必要な OS では test 前に Graphviz を install する
- **THEN** 固定 `plantuml.jar` を保存領域（cache）へ事前取得し、checksum を確認する
- **THEN** crate package に `plantuml.jar.sha256` が含まれ、`plantuml.jar` 本体が含まれないことを確認する
- **THEN** sequence fixture を SVG に描画する
- **THEN** SVG 生成が必要な CI では raw code fallback を成功扱いせず job failure にする
