## 0. Spike Handoff

- [x] 0.1 `spike/plantuml-jvm-embedding` を remote に push 済みであることを確認し、local branch を閉じる
- [x] 0.2 JVM 埋め込み spike の結果を `proposal.md`、`design.md`、delta spec に反映する
- [x] 0.3 spike で出力した SVG が `<svg`、`viewBox`、主要 label を含むことを確認し、実装判断の根拠として記録する
- [x] 0.4 spike SVG の背景、文字、線、矢印が KatanA 表示にそのまま使える品質ではないことを theme 要件へ反映する

## 1. Scope And Existing Behavior

- [x] 1.1 KDV issue #9 の主張を、KDR が PlantUML 描画責務を担うべき問題として整理する
- [x] 1.2 KatanA 側の既存 PlantUML 実装を棚卸しし、JAR 探索、Java 起動、theme 注入、Windows no-console の挙動を KDR 側の設計へ写す
- [x] 1.3 `plantuml.jar` は crate package に本体を含めず、checksum manifest と cache prefetch recipe で扱う方針を決める
- [x] 1.4 KDV issue #9 の受け入れ条件を KDR 側の renderer / CLI / fixture / error contract に分解したうえで、実装順に並べる

## 2. Public API And Renderer

- [x] 2.1 `DiagramKind::PlantUml` を KDR の公開 API に追加する
- [x] 2.2 `PlantUmlRenderer` を追加し、`Renderer::render(&RenderInput)` で SVG または raw code string を返す
- [x] 2.3 `RenderOutputFactory` が PlantUML の SVG metadata、runtime version、checksum、profile、diagnostics を扱えるようにする
- [x] 2.4 `libjvm` / JAR 未解決時は警告付き raw code block を返し、PlantUML の invalid source は診断可能な error に mapping する
- [x] 2.5 CLI と公開 API で共通利用する `plantuml-runtime-unavailable` warning を定義し、原因、確認 path、install / env 設定の対処を必ず含める
- [x] 2.6 `RenderInput.context.theme` の dark / light を PlantUML 公式の dark mode に接続し、fingerprint 差分をテストする
- [x] 2.7 SVG 出力後補正や公式表現を劣化させる `skinparam` を増やさず、PlantUML 公式 theme / dark mode で配色を反映する

## 3. Runtime Dependency Resolution

- [x] 3.1 `libjvm` resolver を追加し、`KDR_PLANTUML_JVM`、`JAVA_HOME`、OS 既定候補の順に macOS / Linux / Windows で解決する
- [x] 3.2 PlantUML JAR resolver を追加し、`KDR_PLANTUML_JAR`、`PLANTUML_JAR`、KDR 保存領域（cache）の順に解決する
- [x] 3.3 `plantuml.jar` の version / URL / checksum manifest を追加し、明示 path と cache path を検証する
- [x] 3.4 runtime asset update / latest check recipe に PlantUML JAR を追加する
- [x] 3.5 runtime package check に PlantUML checksum manifest の含有と JAR 本体の非含有検査を追加する
- [x] 3.6 保存領域（cache）に JAR が無い場合は固定 URL から初回取得し、network / checksum / write 失敗を warning 付き raw fallback にする

## 4. JVM Embedding And Platform Care

- [x] 4.1 Rust から JVM を起動して PlantUML の SVG 生成 API を呼べることを macOS spike で確認する
- [x] 4.2 spike 結果を前提に、JNI で JVM 埋め込み実装を進める
- [x] 4.3 macOS / Linux / Windows で `KDR_PLANTUML_JVM`、`JAVA_HOME`、`libjvm` path、classpath、`plantuml.jar` の解決方法を整理し、CI smoke check に反映する
- [/] 4.4 JVM の一度だけ起動する制約、再初期化不可、thread safety、Java 例外 mapping を KDR の renderer 契約に収める。timeout と crash 隔離は process 内 JVM では保証しないため後続判断に残す
- [x] 4.5 JVM 埋め込みを採る場合の package / CI / KDV consumer への追加設定を整理する
- [x] 4.6 JVM 埋め込みを採用したため、background process facade は初期実装に追加しない判断を記録する
- [x] 4.7 JVM 埋め込みを採用したため、Windows の `CREATE_NO_WINDOW` は子 process 案に限定する
- [x] 4.8 JVM 埋め込みを採用したため、`javaw.exe`、`cmd`、PowerShell wrapper、`java -jar` 経路を追加しない
- [x] 4.9 path に空白を含む Java / JAR path の unit test を追加する
- [x] 4.10 採用した実行経路は JNI のため、PlantUML 用の direct `Command::new` や shell wrapper を追加しない

## 5. CLI And Reference Commands

- [x] 5.1 `kdr plantuml render --input --output --runtime --theme --theme-from --theme-mode` を追加する
- [x] 5.1.1 `kdr plantuml render --help` に指定できる theme 名と `dark` / `light` mode 候補を表示する
- [x] 5.1.2 `kdr plantuml render --cache-dir` を追加し、API と同じ保存領域（cache directory）指定へ変換する
- [x] 5.2 `kdr plantuml reference-update --fixtures` を追加する
- [x] 5.3 `kdr plantuml compare --fixtures --min-score` を追加する
- [x] 5.4 `kdr plantuml bench --fixtures` を追加する
- [x] 5.5 `ReferenceCommand` と Justfile に `plantuml-render`、`plantuml-reference`、`plantuml-compare`、`plantuml-compare-ci`、`plantuml-bench` を追加する
- [x] 5.6 `kdr plantuml render` は raw output で正常終了する場合も、標準エラー出力に「何が起きたか」と「何を設定すべきか」を表示する

## 6. Fixtures And Tests

- [x] 6.1 `tests/fixtures/plantuml/representative/` に sequence / class / activity の代表 fixture を追加する。class fixture は class / abstract class / interface / enum / note / 関係線 / 属性 / メソッドを含める
- [x] 6.1.1 公式ドキュメント相当の sequence / use-case / class / object / activity / component / deployment / state / timing の 9 fixture を `tests/fixtures/plantuml/official/` に追加し、抜粋方法と公式 URL 対応表を README に残す
- [x] 6.2 renderer unit test で sequence / class / activity が `<svg` を返すことを検証する
- [x] 6.3 SVG の背景、文字、線、矢印、participant、class、note が PlantUML 公式 dark mode の色変換に由来することを検証する
- [x] 6.3.1 class icon と visibility icon の PlantUML 公式表現を消さないことを検証する
- [x] 6.4 JAR 未検出、`libjvm` 未検出、PlantUML invalid source の error mapping test を追加する
- [x] 6.5 `libjvm` 未検出時に warning を返す resolver test と raw code block fallback test を追加する
- [x] 6.6 API fallback test で `tracing::warn` と `RenderOutput.diagnostics` に同じ warning text を渡す経路を確認する
- [x] 6.7 CLI fallback test で標準エラー出力に原因と対処が出て、標準出力または output file に raw code block が出ることを確認する
- [x] 6.8 CLI parse / render test に `plantuml` subcommand を追加する
- [x] 6.9 theme 指定 test で `!theme` config と cache fingerprint の差分を検証する
- [x] 6.9.1 `RenderInput.config.vendor_config.plantuml_theme` と CLI `--theme` / `--theme-from` で PlantUML 公式 theme を切り替えられることを検証する
- [x] 6.9.2 API から指定できる PlantUML theme 名一覧を取得できることを検証する
- [x] 6.9.3 CLI help に PlantUML theme 名一覧と `--theme-mode dark|light` が出ることを検証する
- [x] 6.9.4 API の `plantuml_cache_dir` / `plantumlCacheDir` と `RuntimePathResolver::resolve_with_plantuml_cache_dir` で保存領域（cache directory）を上書きでき、保存領域だけでは `cache_fingerprint` が変わらないことを検証する
- [x] 6.10 package size / license 方針に従い、JAR 本体を package に含めず cache runtime asset として検証する

## 7. Multi Platform CI

- [x] 7.1 GitHub Actions の macOS job で Java 解決、JAR checksum、sequence SVG smoke check を実行する
- [x] 7.2 GitHub Actions の Ubuntu job で Java 解決、JAR checksum、sequence SVG smoke check を実行する
- [x] 7.3 GitHub Actions の Windows job で Java 解決、JAR checksum、sequence SVG smoke check を実行する
- [x] 7.4 CI の通常経路では PlantUML reference を再生成せず、git 管理済み fixture と KDR 出力だけを比較する

## 8. Downstream Handoff

- [x] 8.1 KDV が `kdr-plantuml` / `plantuml-render` gate を `ExternalBackendRequired` から外すために必要な KDR tag / API / error contract をまとめる
- [x] 8.2 KatanA 側に残る `katana-plantuml` backend を KDR adapter へ寄せる後続 change の範囲を記録する
- [x] 8.3 KDR が `<figure data-kdv-diagram="plantuml">` を生成しないことを KDV handoff に明記する

## 9. Verification For Implementation Session

- [x] 9.1 `just plantuml-render tests/fixtures/plantuml/representative tmp/kdr-plantuml-rendered` を実行する
- [x] 9.2 `just plantuml-compare-ci 100` を公式 9 fixture で実行し、全件 100 点を確認する
- [x] 9.3 `just runtime-asset-check` を実行する
- [x] 9.4 `just runtime-bundle-package-check` または PlantUML runtime package check を実行する
- [x] 9.5 `just ast-lint` を実行する
- [x] 9.6 `just unit-test` を実行する
- [x] 9.7 `./scripts/openspec validate v0-2-0-plantuml-svg-renderer-multiplatform --strict` を実行する
- [x] 9.8 `/self-review` を実行し、OpenSpec、実装差分、検証結果の整合を確認する

## 10. OpenSpec Handoff

- [x] 10.1 この OpenSpec change を validate する
- [x] 10.2 OpenSpec 以外の実装差分を commit に含めていないことを確認する
- [x] 10.3 OpenSpec change の commit / push を行い、移譲先 session が実装に入れる状態にする

## 11. Feedback Follow-up

- [x] 11.1 PlantUML 公式 dark mode が `FileFormatOption` の `ColorMapper.DARK_MODE` で JVM 埋め込みから利用できることを確認する
- [x] 11.2 PlantUML 公式 theme は `!theme` config として渡し、KDR 独自の色上書きではなく PlantUML 側の theme 管理を使う
- [x] 11.3 `plantuml-reference` は公式 PlantUML CLI の dark mode 出力から参照 PNG を生成し、`plantuml-compare` は Mermaid / Draw.io と同じ画像比較スコアを出す
- [x] 11.4 公式 9 fixture の比較で minimum score 100.00 を確認する
- [x] 11.5 公式 9 fixture の score gate を 100 に固定し、100 未満への劣化を検知する
