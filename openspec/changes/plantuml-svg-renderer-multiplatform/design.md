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
- 実行時に network から `plantuml.jar` を暗黙 download すること
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

### 3. `plantuml.jar` は version / checksum 固定の runtime asset として扱う

`plantuml.jar` は KDR の runtime asset 管理対象に入れ、固定 version、checksum、取り込み recipe、package 含有検査を持つ。
実行時は repository / package に含まれる JAR を優先し、`KDR_PLANTUML_JAR` と既存互換の `PLANTUML_JAR` で上書きできる。

代替案は、利用者が system install した PlantUML だけに依存すること。CI や KDV export で再現性が落ちるため採らない。

### 4. JVM 埋め込みを初期実装の第一候補にする

初期版では、Rust process 内に Java 仮想マシン（JVM: Java Virtual Machine）を埋め込む案を第一候補にする。
KatanA 側では PlantUML 実行のために Windows の headless process 制御（端末窓を出さない制御）まで持っていた。KDR が PlantUML 描画責務を引き取るなら、その process 制御も KDR に移る。ここを単純移植すると KatanA と KDR の両方で外部 process 管理を重複管理することになる。

そのため初期実装では、JNI（Java Native Interface: Rust など外部言語から JVM を呼ぶ仕組み）または同等の JVM 埋め込み方式を使い、少なくとも次を満たすことを確認する。

- macOS / Linux / Windows で `libjvm` を安定して見つけられるか
- `plantuml.jar` を classpath に渡して SVG 生成 API を呼べるか
- JVM の一度だけ起動する制約、再初期化不可、thread safety を KDR の renderer 契約に収められるか
- Java 側の例外、stderr 相当、timeout、panic / crash 隔離をどう診断へ戻すか
- crates.io package、GitHub Actions、KDV consumer で追加設定がどの程度必要か

JVM 埋め込みが OS 差分や crash 隔離の条件を満たせない場合に限り、子 process 実行を fallback ではなく明示的な採用案として使う。

#### Current Spike Evidence

`spike/plantuml-jvm-embedding` では、macOS ローカルで `jni` crate の `invocation` feature を使い、Rust から JVM を起動して `net.sourceforge.plantuml.SourceStringReader` を呼び、`plantuml.jar` から SVG を取得できることを確認した。
検証は `crates/katana-diagram-renderer/tests/plantuml_jvm_spike.rs` の `source_string_reader_generates_svg_through_embedded_jvm` で行い、`cargo test -p katana-diagram-renderer --test plantuml_jvm_spike -- --nocapture` が成功した。
出力された SVG は `tmp/plantuml-jvm-spike-svg/embedded-sequence.svg` で確認し、`<svg`、`viewBox`、`Alice`、`Bob`、`hello` を含んでいたため、PlantUML の SVG 描画経路として成立している。
この spike branch は `origin/spike/plantuml-jvm-embedding` に残し、local branch は閉じる。
未確認の論点は Linux / Windows での `libjvm` 解決、JVM の lifetime を process 全体で1回に固定する設計、timeout と crash 隔離である。
`libjvm` を認識できない環境は失敗扱いではなく、警告付き raw code block への早期 return とする。

### 5. 子 process 実行を採る場合でも shell wrapper を通さない

JVM 埋め込みを採らない場合、KDR 内に process facade を置き、`std::process::Command` を直接散らさない。Windows では `CREATE_NO_WINDOW` 相当の flag を facade に集約する。
実行は `java -Djava.awt.headless=true -jar ... -pipe -tsvg` とし、stdin / stdout / stderr を pipe する。

代替案は `cmd`、PowerShell、`javaw.exe` への分岐。引数 escape、stdout pipe、端末窓抑制の失敗が起きやすいため採らない。

### 6. テーマ反映は `RenderInput.context.theme` を正とする

既存 KatanA の `skinparam` 注入相当を KDR に移し、`RenderInput.context.theme` がある場合はその値から PlantUML の色指定を組み立てる。
process global な dark mode は fallback に限定する。
spike の SVG は図形としては成立したが、背景や文字色が KatanA 表示にそのまま使える品質ではなかった。
そのため PlantUML も Mermaid / Draw.io と同じく、KDR の theme snapshot を唯一の入力として扱う。
初期実装では少なくとも背景、既定文字、線、矢印、participant、class、note、activity node の色を `skinparam` で制御する。
PlantUML 側の制約で SVG 生成後にしか直せない属性がある場合だけ、Draw.io と同じ考え方で最小限の SVG 補正を入れる。

代替案は PlantUML 既定テーマに任せること。KDV / KatanA の light / dark export で Mermaid / Draw.io と揃わないため採らない。

## Risks / Trade-offs

- [Risk] JVM 埋め込みは `libjvm` 探索、JVM の寿命管理、crash 隔離が難しい → macOS spike の成立を前提に実装へ進み、macOS / Linux / Windows の CI と smoke check で採用条件を確認する
- [Risk] Java が無い環境では PlantUML が図にならない → KDR の診断に `libjvm` path、JAR path、解決順、install hint を含め、raw code block を返して表示自体は継続する
- [Risk] `plantuml.jar` を package に含めると配布サイズや license 確認が必要になる → 実装前に crate package size と license を task で確認し、含有できない場合は KDR 管理 cache 方式を同じ契約内で明記する
- [Risk] Windows の端末窓が再発する → process facade と AST lint で PlantUML 実行が facade を通ることを検査する
- [Risk] 公式 PlantUML SVG は OS / Java version で細かく変わる可能性がある → 最小 fixture は構造検査を主にし、reference score は platform 差分を許容する基準を別途持つ
- [Risk] KatanA から KDR へ責務を移す途中で二重実装になる → KDR 側完了後に KDV / KatanA adapter の削除範囲を後続 change で扱う

## Migration Plan

1. KDR に PlantUML renderer / CLI / runtime asset 契約を追加する
2. macOS / Ubuntu / Windows の CI で Java 検出、JAR 解決、最小 SVG 生成を確認する
3. KDR tag を KDV が consume し、`ExternalBackendRequired` を外す
4. KatanA 側に残る PlantUML renderer を KDR adapter 経由へ寄せる後続作業を作る

Rollback は、KDV / KatanA 側の KDR version pin を戻し、KDR 内の PlantUML change を archive せずに修正継続する。

## Open Questions

- `plantuml.jar` を crates.io package に含められるか。含められない場合は、KDR 管理 cache と explicit install command を正規経路にする。
- reference score を Mermaid / Draw.io と同じ画像比較にするか、初期段階では SVG 構造検査と smoke check に留めるか。
