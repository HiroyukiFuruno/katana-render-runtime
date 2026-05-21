## Context

KDR は Mermaid / Draw.io / ZenUML の外部図形描画を KatanA から切り出した repository であり、`openspec/project.md` でも PlantUML は外部描画責務に含まれている。
一方で現在の KDR 公開 API は `DiagramKind::Mermaid` と `DiagramKind::Drawio` だけを扱い、KDV は PlantUML を KDR 経由で SVG 化できない。

KatanA 側には既存の PlantUML 経路があり、主な挙動は次の通り。

- `PLANTUML_JAR`、Homebrew path、実行ファイル隣接 path、ユーザー local path から `plantuml.jar` を探す
- `java -Djava.awt.headless=true -jar <plantuml.jar> -pipe -tsvg` に source を stdin で渡し、SVG を stdout から受け取る
- Windows では `CREATE_NO_WINDOW` を付ける process facade を通して、黒い端末窓を出さない
- `javaw.exe` は stdout / stderr pipe と相性が悪いため、`java.exe` と headless option を使う

この change は、KatanA 側に残っている PlantUML 描画責務を KDR に戻す計画である。KDV は描画結果の埋め込みだけを担当する。

## Goals / Non-Goals

**Goals:**

- KDR 公開 API と CLI から PlantUML を SVG 描画できるようにする
- `libjvm` や `plantuml.jar` が認識できない場合は、警告付きで raw code block を返し、viewer / export を止めない
- CLI と公開 API の両方で、警告ログに原因、確認した path、利用者が次に行うべき install / env 設定を明示する
- sequence / class / activity の最小 fixture を KDR 内で自動検証する
- Java 実行環境と `plantuml.jar` の依存を macOS / Linux / Windows で診断可能にする
- Windows で PlantUML 実行時に端末窓を出さない
- KDV が `ExternalBackendRequired` を返さず、KDR の成功 / 失敗をそのまま扱える契約にする

**Non-Goals:**

- KDR が HTML export、PDF export、viewer UI を持つこと
- Java 実行環境（JRE: Java Runtime Environment）を KDR package に同梱すること
- KDV 側 adapter の実装まで同じ change で行うこと

## Decisions

### 1. PlantUML 描画責務は KDR に置く

`DiagramKind::PlantUml`、`PlantUmlRenderer`、`kdr plantuml ...` を追加し、Mermaid / Draw.io と同じ renderer 契約で SVG を返す。
KDV 側で独自に Java / JAR を探す実装は増やさない。

代替案は、KDV 側に PlantUML 専用 backend を残すこと。これは KDR の責務境界を崩し、KDV v0.1.0 の HTML export 完了条件を KDV 内の外部 process 実装に依存させるため採らない。

### 2. Java 実行環境は同梱せず、検出と degraded 表示を KDR が持つ

KDR は `libjvm` の検出、`JAVA_HOME`、`KDR_PLANTUML_JVM` による上書き、失敗時の診断を持つ。
`libjvm` または `plantuml.jar` が認識できない場合、エラーで viewer / export を止めず、警告（warning）を diagnostics に入れたうえで raw code block を返す。
この警告は内部だけに閉じない。KDR は共通の診断イベントを生成し、CLI は標準エラー出力（stderr）へ表示し、公開 API はロガーと `RenderOutput.diagnostics` の両方で渡す。
警告には `kind=plantuml-runtime-unavailable`、原因、確認した環境変数と path、次に行うべき Java 実行環境または `plantuml.jar` の install / path 設定を含める。
PlantUML を図として使いたい利用者には、Java 実行環境を install してもらう。

代替案は JRE 同梱。配布サイズ、OS 別 package、更新責務が大きくなり、KDR の図形描画 library としての責務を超えるため採らない。

### 3. `plantuml.jar` は version / checksum 固定の cache runtime asset として扱う

`plantuml.jar` は KDR の runtime asset 管理対象に入れ、固定 version、download URL、checksum、latest check、prefetch recipe、update recipe、package 検査を持つ。
JAR 本体は crates.io の package size 上限を超えるため crate package に含めず、checksum manifest だけを crate package に含める。
KDR は既定では OS 別の保存領域（cache）の固定 path を確認し、存在する JAR の checksum を検証してから JVM classpath に渡す。
保存領域（cache）に JAR が無い場合は固定 URL から初回 download し、checksum 検証後に配置する。
network が使えない、保存領域（cache）へ書き込めない、または checksum が一致しない場合は raw fallback と warning で継続し、利用者に network 接続、`KDR_PLANTUML_CACHE_DIR` / API `plantuml_cache_dir`、または `KDR_PLANTUML_JAR` 設定を促す。
実行時は `KDR_PLANTUML_JAR`、既存互換の `PLANTUML_JAR` が明示された場合だけそれを優先し、それ以外は KDR の保存領域（cache）を使う。
明示 path は利用者管理の JAR として扱い、存在確認と checksum 検証を行う。

保存領域（cache）の既定値は、macOS では `~/Library/Caches/kdr/plantuml`、Linux では `$XDG_CACHE_HOME/kdr/plantuml` または `~/.cache/kdr/plantuml`、Windows では `%LOCALAPPDATA%\kdr\plantuml` とする。
CLI は `--cache-dir`、公開 API は `RenderInput.config.vendor_config.plantuml_cache_dir` / `plantumlCacheDir` と `RuntimePathResolver::resolve_with_plantuml_cache_dir` で同じ上書き経路を持つ。
保存領域（cache）の違いは描画内容ではないため、`cache_fingerprint` には含めない。

代替案は、利用者が system install した PlantUML だけに依存すること。CI や KDV export で再現性が落ちるため採らない。

### 4. JVM 埋め込みを初期実装の第一候補にする

初期版では、Rust process 内に Java 仮想マシン（JVM: Java Virtual Machine）を埋め込む案を第一候補にする。
KatanA 側では PlantUML 実行のために Windows の headless process 制御（端末窓を出さない制御）まで持っていた。KDR が PlantUML 描画責務を引き取るなら、その process 制御も KDR に移る。ここを単純移植すると KatanA と KDR の両方で外部 process 管理を重複管理することになる。

そのため初期実装では、JNI（Java Native Interface: Rust など外部言語から JVM を呼ぶ仕組み）または同等の JVM 埋め込み方式を使い、少なくとも次を満たすことを確認する。

- macOS / Linux / Windows で `libjvm` を安定して見つけられるか
- `plantuml.jar` を classpath に渡して SVG 生成 API を呼べるか
- JVM の一度だけ起動する制約、再初期化不可、thread safety を KDR の renderer 契約に収められるか
- Java 側の例外、stderr 相当、timeout、panic / crash 隔離をどう診断へ戻すか
- crates.io package、GitHub Actions、KDV consumer で cache prefetch がどの程度必要か

JVM 埋め込みが OS 差分や crash 隔離の条件を満たせない場合に限り、子 process 実行を fallback ではなく明示的な採用案として使う。

#### Current Spike Evidence

`spike/plantuml-jvm-embedding` では、macOS ローカルで `jni` crate の `invocation` feature を使い、Rust から JVM を起動して `net.sourceforge.plantuml.SourceStringReader` を呼び、`plantuml.jar` から SVG を取得できることを確認した。
検証は `crates/katana-diagram-renderer/tests/plantuml_jvm_spike.rs` の `source_string_reader_generates_svg_through_embedded_jvm` で行い、`cargo test -p katana-diagram-renderer --test plantuml_jvm_spike -- --nocapture` が成功した。
出力された SVG は `tmp/plantuml-jvm-spike-svg/embedded-sequence.svg` で確認し、`<svg`、`viewBox`、`Alice`、`Bob`、`hello` を含んでいたため、PlantUML の SVG 描画経路として成立している。
この spike branch は `origin/spike/plantuml-jvm-embedding` に残し、local branch は閉じる。
未確認の論点は Linux / Windows での `libjvm` 解決、JVM の lifetime を process 全体で1回に固定する設計、timeout と crash 隔離である。
`libjvm` を認識できない環境は失敗扱いではなく、警告付き raw code block への早期 return とする。

正式実装では JVM 埋め込みを採用した。
KDR は process 内で JVM を一度だけ起動し、同じ `plantuml.jar` に対する再利用、Java 例外の `RenderError::Runtime` への変換、別 JAR での再初期化拒否、`libjvm` / JAR 不足時の raw fallback を実装する。
一方、timeout と crash 隔離は process 内 JVM では強く保証できないため、初期版の完了条件には含めず、必要になった時点で子 process 実行案として再評価する。

### 5. 子 process 実行を採る場合でも shell wrapper を通さない

JVM 埋め込みを採らない場合、KDR 内に process facade を置き、`std::process::Command` を直接散らさない。Windows では `CREATE_NO_WINDOW` 相当の flag を facade に集約する。
実行は `java -Djava.awt.headless=true -jar ... -pipe -tsvg` とし、stdin / stdout / stderr を pipe する。

代替案は `cmd`、PowerShell、`javaw.exe` への分岐。引数 escape、stdout pipe、端末窓抑制の失敗が起きやすいため採らない。

### 6. テーマ反映は PlantUML 公式 theme / dark mode を正とする

PlantUML は公式に theme と dark mode を持つため、KDR は独自の大きな `skinparam` 上書きを既定経路にしない。
`RenderInput.context.theme` の dark / light は、JVM 埋め込みでは PlantUML の `FileFormatOption` に `ColorMapper.DARK_MODE` を渡すことで反映する。
CLI の `--theme` / `--theme-from` と API の `RenderInput.config.vendor_config.plantuml_theme` / `plantuml_theme_from` は、PlantUML の `!theme <name>` / `!theme <name> from <path-or-url>` として渡す。
CLI の `--theme-mode dark|light` と API の `plantuml_theme_mode` は、KatanA の theme snapshot を使えない呼び出し元でも PlantUML 公式 dark mode を明示できる入口にする。
CLI help は固定 PlantUML runtime が持つ theme 名一覧を表示し、公開 API は `PlantUmlRenderer::available_themes()` と `PlantUmlThemeCatalog::names()` で同じ一覧を返す。
PlantUML 公式 theme は PlantUML 側の repository と JAR に管理されるため、KDR は theme file を複製せず、公式の directive に接続する。

PlantUML は外部描画 engine の出力を正とし、Mermaid / Draw.io のような SVG 後加工は原則として行わない。
class icon や visibility icon のような PlantUML 公式の意味色は消さない。
`hide circle`、`classAttributeIconSize 0`、monochrome 化のように公式表現を劣化させる指定は初期実装に入れない。

代替案は KatanA 互換色を `skinparam` で細かく再定義すること。これは公式 dark mode / theme の意味色と乖離し、class / visibility icon を劣化させるため採らない。

### 7. 公式 9 種類を画像比較スコアで評価する

PlantUML の smoke test は `<svg` の有無だけでは不十分である。
`tests/fixtures/plantuml/official/` に公式ドキュメント相当の sequence / use-case / class / object / activity / component / deployment / state / timing の 9 種類を置く。
`plantuml-reference` は公式 PlantUML CLI の dark mode 出力を参照 SVG / PNG として生成する。
`plantuml-compare` は KDR 出力を同じ条件で画像化し、Mermaid / Draw.io と同じ reference score で比較する。
初期の合格ラインは 100 点とし、公式 9 種類すべてで 100 点を満たすことを完了条件にする。

## Risks / Trade-offs

- [Risk] JVM 埋め込みは `libjvm` 探索、JVM の寿命管理、crash 隔離が難しい → macOS spike の成立を前提に実装へ進み、macOS / Linux / Windows の CI と smoke check で採用条件を確認する。timeout と crash 隔離は初期版では保証しない
- [Risk] Java が無い環境では PlantUML が図にならない → KDR の診断に `libjvm` path、JAR path、解決順、install hint を含め、raw code block を返して表示自体は継続する
- [Risk] `plantuml.jar` を package に含めると crates.io の package size 上限を超える → JAR 本体は package に含めず、固定 version / download URL / checksum manifest / cache prefetch recipe で管理する
- [Risk] Windows の端末窓が再発する → 正式実装では子 process を起動しない JVM 埋め込みを採用したため、端末窓を出す実行経路を持たない。子 process 案へ切り替える場合だけ process facade と AST lint を追加する
- [Risk] 公式 PlantUML SVG は OS / Java version で細かく変わる可能性がある → 公式 9 fixture は同じ環境で公式 CLI 出力と KDR 出力を画像化し、100 点未満になった時点で劣化として検出する
- [Risk] KatanA から KDR へ責務を移す途中で二重実装になる → KDR 側完了後に KDV / KatanA adapter の削除範囲を後続 change で扱う

## Migration Plan

1. KDR に PlantUML renderer / CLI / runtime asset 契約を追加する
2. macOS / Ubuntu / Windows の CI で Java 検出、JAR 解決、最小 SVG 生成を確認する
3. KDR tag を KDV が consume し、`ExternalBackendRequired` を外す
4. KatanA 側に残る PlantUML renderer を KDR adapter 経由へ寄せる後続作業を作る

Rollback は、KDV / KatanA 側の KDR version pin を戻し、KDR 内の PlantUML change を archive せずに修正継続する。

## Open Questions

- timeout と crash 隔離が必要になった場合、JVM 埋め込みを維持するか、子 process 実行へ切り替えるか。
