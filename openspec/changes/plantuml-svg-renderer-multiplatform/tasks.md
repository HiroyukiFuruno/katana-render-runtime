## 0. Spike Handoff

- [/] 0.1 `spike/plantuml-jvm-embedding` を remote に push 済みであることを確認し、local branch を閉じる
- [x] 0.2 JVM 埋め込み spike の結果を `proposal.md`、`design.md`、delta spec に反映する
- [x] 0.3 spike で出力した SVG が `<svg`、`viewBox`、主要 label を含むことを確認し、実装判断の根拠として記録する
- [/] 0.4 spike SVG の背景、文字、線、矢印が KatanA 表示にそのまま使える品質ではないことを theme 要件へ反映する

## 1. Scope And Existing Behavior

- [x] 1.1 KDV issue #9 の主張を、KDR が PlantUML 描画責務を担うべき問題として整理する
- [x] 1.2 KatanA 側の既存 PlantUML 実装を棚卸しし、JAR 探索、Java 起動、theme 注入、Windows no-console の挙動を KDR 側の設計へ写す
- [ ] 1.3 `plantuml.jar` を crate package に含められるか、package size、license、配布方針を確認し、含められない場合の KDR 管理 cache 方針を決める
- [ ] 1.4 KDV issue #9 の受け入れ条件を KDR 側の renderer / CLI / fixture / error contract に分解したうえで、移譲先 session の実装順に並べる

## 2. Public API And Renderer

- [ ] 2.1 `DiagramKind::PlantUml` を KDR の公開 API に追加する
- [ ] 2.2 `PlantUmlRenderer` を追加し、`Renderer::render(&RenderInput)` で SVG または raw code string を返す
- [ ] 2.3 `RenderOutputFactory` が PlantUML の SVG metadata、runtime version、checksum、profile、diagnostics を扱えるようにする
- [ ] 2.4 `libjvm` / JAR 未解決時は警告付き raw code block を返し、PlantUML の invalid source は診断可能な error に mapping する
- [ ] 2.5 CLI と公開 API で共通利用する `plantuml-runtime-unavailable` warning を定義し、原因、確認 path、install / env 設定の対処を必ず含める
- [ ] 2.6 `RenderInput.context.theme` から PlantUML `skinparam` を生成し、light / dark の fingerprint 差分をテストする
- [ ] 2.7 SVG 出力後にしか補正できない色属性がある場合だけ、Draw.io と同じ最小補正を入れる

## 3. Runtime Dependency Resolution

- [ ] 3.1 `libjvm` resolver を追加し、`KDR_PLANTUML_JVM`、`JAVA_HOME`、OS 既定候補の順に macOS / Linux / Windows で解決する
- [ ] 3.2 PlantUML JAR resolver を追加し、`KDR_PLANTUML_JAR`、`PLANTUML_JAR`、package 内 path、実行ファイル隣接 path、user local path を順に解決する
- [ ] 3.3 `plantuml.jar` の version / checksum manifest を追加し、明示 path と managed path の検証方針を分ける
- [ ] 3.4 runtime asset update / latest check recipe に PlantUML JAR を追加する
- [ ] 3.5 runtime package check に PlantUML JAR と checksum manifest の含有検査を追加する

## 4. JVM Embedding And Platform Care

- [x] 4.1 Rust から JVM を起動して PlantUML の SVG 生成 API を呼べることを macOS spike で確認する
- [ ] 4.2 spike 結果を前提に、JNI または同等方式で JVM 埋め込み実装を進める
- [ ] 4.3 macOS / Linux / Windows で `KDR_PLANTUML_JVM`、`JAVA_HOME`、`libjvm` path、classpath、`plantuml.jar` の解決方法を比較する
- [ ] 4.4 JVM の一度だけ起動する制約、再初期化不可、thread safety、timeout、Java 例外 mapping を KDR の renderer 契約に収める
- [ ] 4.5 JVM 埋め込みを採る場合の package / CI / KDV consumer への追加設定を整理する
- [ ] 4.6 JVM 埋め込みが multi platform 条件を満たせない場合だけ、KDR 内に background process facade を追加する
- [ ] 4.7 子 process 実行を採る場合、Windows では `CREATE_NO_WINDOW` 相当を facade に入れ、PlantUML 実行時に端末窓を出さない
- [ ] 4.8 子 process 実行を採る場合、`javaw.exe`、`cmd`、PowerShell wrapper を使わず、`java.exe` / `java` に `-Djava.awt.headless=true -jar <jar> -pipe -tsvg` を渡す
- [ ] 4.9 path に空白を含む Java / JAR path の unit test を追加する
- [ ] 4.10 AST lint または構造検査で、採用した実行経路以外の direct `Command::new` や shell wrapper を検出する

## 5. CLI And Reference Commands

- [ ] 5.1 `kdr plantuml render --input --output --runtime` を追加する
- [ ] 5.2 `kdr plantuml reference-update --fixtures` を追加する
- [ ] 5.3 `kdr plantuml compare --fixtures --min-score` を追加する
- [ ] 5.4 `kdr plantuml bench --fixtures` を追加する
- [ ] 5.5 `ReferenceCommand` と Justfile に `plantuml-render`、`plantuml-reference`、`plantuml-compare`、`plantuml-compare-ci`、`plantuml-bench` を追加する
- [ ] 5.6 `kdr plantuml render` は raw output で正常終了する場合も、標準エラー出力に「何が起きたか」と「何を設定すべきか」を表示する

## 6. Fixtures And Tests

- [ ] 6.1 `tests/fixtures/plantuml/representative/` に sequence / class / activity の最小 fixture を追加する
- [ ] 6.2 renderer unit test で sequence / class / activity が `<svg` を返すことを検証する
- [ ] 6.3 SVG の背景、文字、線、矢印、participant、class、note、activity node が KatanA theme に由来することを検証する
- [ ] 6.4 Java 未検出、JAR 未検出、PlantUML invalid source の error mapping test を追加する
- [ ] 6.5 `libjvm` 未検出時に warning と raw code block を返す fallback test を追加する
- [ ] 6.6 API fallback test で warning log と `RenderOutput.diagnostics` の両方に同じ warning code が出ることを確認する
- [ ] 6.7 CLI fallback test で標準エラー出力に原因と対処が出て、標準出力または output file に raw code block が出ることを確認する
- [ ] 6.8 CLI parse / render test に `plantuml` subcommand を追加する
- [ ] 6.9 theme 注入 test で `skinparam` と cache fingerprint の差分を検証する
- [ ] 6.10 package size / license 方針に従い、JAR を fixture test にだけ置くのではなく runtime asset として検証する

## 7. Multi Platform CI

- [ ] 7.1 GitHub Actions の macOS job で Java 解決、JAR checksum、sequence SVG smoke check を実行する
- [ ] 7.2 GitHub Actions の Ubuntu job で Java 解決、JAR checksum、sequence SVG smoke check を実行する
- [ ] 7.3 GitHub Actions の Windows job で Java 解決、JAR checksum、sequence SVG smoke check を実行する
- [ ] 7.4 CI の通常経路では PlantUML reference を再生成せず、git 管理済み reference と KDR 出力だけを比較する

## 8. Downstream Handoff

- [ ] 8.1 KDV が `kdr-plantuml` / `plantuml-render` gate を `ExternalBackendRequired` から外すために必要な KDR tag / API / error contract をまとめる
- [ ] 8.2 KatanA 側に残る `katana-plantuml` backend を KDR adapter へ寄せる後続 change の範囲を記録する
- [ ] 8.3 KDR が `<figure data-kdv-diagram="plantuml">` を生成しないことを KDV handoff に明記する

## 9. Verification For Implementation Session

- [ ] 9.1 `just plantuml-render tests/fixtures/plantuml/representative tmp/kdr-plantuml-rendered` を実行する
- [ ] 9.2 `just plantuml-compare-ci 99` を実行する
- [ ] 9.3 `just runtime-asset-check` を実行する
- [ ] 9.4 `just runtime-bundle-package-check` または PlantUML JAR package check を実行する
- [ ] 9.5 `just ast-lint` を実行する
- [ ] 9.6 `just unit-test` を実行する
- [ ] 9.7 `./scripts/openspec validate plantuml-svg-renderer-multiplatform --strict` を実行する
- [ ] 9.8 `/self-review` を実行し、OpenSpec、実装差分、検証結果の整合を確認する

## 10. OpenSpec Handoff

- [x] 10.1 この OpenSpec change を validate する
- [x] 10.2 OpenSpec 以外の実装差分を commit に含めていないことを確認する
- [ ] 10.3 OpenSpec change の commit / push を行い、移譲先 session が実装に入れる状態にする
