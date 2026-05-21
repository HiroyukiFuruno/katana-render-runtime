# plantuml-svg-rendering Specification

## Purpose
TBD - created by archiving change v0-2-0-plantuml-svg-renderer-multiplatform. Update Purpose after archive.
## Requirements
### Requirement: PlantUML source を SVG または raw code として返さなければならない

システムは、`DiagramKind::PlantUml` の `RenderInput` を受け取り、PlantUML source を SVG または raw code string として返さなければならない（MUST）。`libjvm` または `plantuml.jar` が認識できない場合、警告（warning）をログと diagnostics に含め、raw code block を返さなければならない（MUST）。

#### Scenario: sequence diagram を描画する

- **GIVEN** `@startuml` / `@enduml` を含む sequence diagram source
- **WHEN** `PlantUmlRenderer::render(&RenderInput)` を呼ぶ
- **THEN** `RenderOutput.svg` は `<svg` を含む
- **THEN** `RenderOutput.runtime.name` は PlantUML を識別できる
- **THEN** `RenderOutput.cache_fingerprint` は source、runtime version、theme に基づいて変化する

#### Scenario: class diagram を描画する

- **GIVEN** `class PreviewPane` を含む class diagram source
- **WHEN** `kdr plantuml render --input <file> --output <file>` を実行する
- **THEN** output file には `<svg` と `PreviewPane` を確認できる描画結果が書かれる
- **THEN** raw code だけの出力にならない

#### Scenario: activity diagram を描画する

- **GIVEN** `start` / `stop` を含む activity diagram source
- **WHEN** PlantUML renderer が source を処理する
- **THEN** SVG が返る
- **THEN** process stderr が空でない失敗時は `RenderError::Runtime` または診断可能な error として返る

#### Scenario: libjvm が無い環境で raw code block を返す

- **GIVEN** `libjvm` が認識できない環境
- **WHEN** PlantUML renderer が source を処理する
- **THEN** システムは警告（warning）を diagnostics に含める
- **THEN** システムは警告（warning）をログに出す
- **THEN** 警告には原因、確認した path、利用者が次に行う install / env 設定を含める
- **THEN** システムは早期 return する
- **THEN** 戻り値は raw code block として表示できる string になる
- **THEN** viewer / export は停止しない

### Requirement: PlantUML CLI を提供しなければならない

システムは、`kdr plantuml render`、`kdr plantuml reference-update`、`kdr plantuml compare`、`kdr plantuml bench` を提供しなければならない（MUST）。CLI（command line interface: コマンドで使う入口）は Mermaid / Draw.io と同じ action 構造を使わなければならない（MUST）。

#### Scenario: render command を parse する

- **WHEN** `kdr plantuml render --input in.puml --output out.svg` を parse する
- **THEN** command は `DiagramKind::PlantUml` の render action として扱われる

#### Scenario: render command が raw fallback で正常終了する

- **GIVEN** `libjvm` または `plantuml.jar` が認識できない環境
- **WHEN** `kdr plantuml render --input in.puml` を実行する
- **THEN** CLI は raw code block を標準出力または output file に出す
- **THEN** CLI は終了 code 0 を返す
- **THEN** CLI は標準エラー出力に警告（warning）を出す
- **THEN** 警告には原因、確認した path、利用者が次に行う install / env 設定を含める

#### Scenario: compare command を実行する

- **WHEN** `kdr plantuml compare --fixtures tests/fixtures/plantuml/official --min-score 100` を実行する
- **THEN** PlantUML fixture の公式 dark mode reference と KDR 出力を画像化して比較する
- **THEN** sequence / use-case / class / object / activity / component / deployment / state / timing の 9 種類すべてが 100 点になる
- **THEN** `libjvm` または `plantuml.jar` が解決できない場合は、比較不能として診断付きで失敗する

### Requirement: KDV が埋め込める SVG または code block を返さなければならない

システムは、KDV が HTML export に埋め込める SVG または raw code block を返さなければならない（MUST）。KDR は `<figure data-kdv-diagram="plantuml">` の生成責務を持ってはならない（MUST NOT）。

#### Scenario: KDV が PlantUML SVG を埋め込む

- **GIVEN** KDV が `plantuml` fenced code を KDR に渡す
- **WHEN** KDR が `RenderOutput.svg` を返す
- **THEN** KDV はその SVG を `<figure data-kdv-diagram="plantuml">` 内へ埋め込める
- **THEN** KDR は KDV 固有 HTML wrapper を返さない

#### Scenario: KDV が PlantUML raw code fallback を埋め込む

- **GIVEN** KDR が warning 付き raw code block を返す
- **WHEN** KDV が HTML export を生成する
- **THEN** KDV は raw code block を表示できる
- **THEN** KDV は PlantUML 描画不能を export 全体の失敗として扱わない

### Requirement: PlantUML theme は公式 theme / dark mode として反映しなければならない

システムは、`RenderInput.context.theme` の dark / light を PlantUML 公式 dark mode として反映しなければならない（MUST）。
システムは、API の `RenderInput.config.vendor_config.plantuml_theme` と CLI の `--theme` を PlantUML 公式 `!theme` directive として反映しなければならない（MUST）。
システムは、API の `RenderInput.config.vendor_config.plantuml_theme_from` と CLI の `--theme-from` を PlantUML 公式 local / remote theme source として反映しなければならない（MUST）。
システムは、API の `RenderInput.config.vendor_config.plantuml_theme_mode` と CLI の `--theme-mode` で `dark` / `light` を明示指定できなければならない（MUST）。
システムは、CLI help で固定 PlantUML runtime が提供する theme 名一覧を表示し、公開 API で同じ一覧を返せなければならない（MUST）。
システムは、class icon や visibility icon のような PlantUML 公式の意味表現を消す option を追加してはならない（MUST NOT）。

#### Scenario: light / dark theme で fingerprint が変わる

- **GIVEN** 同一 PlantUML source と light / dark の `RenderInput.context.theme`
- **WHEN** PlantUML renderer をそれぞれ呼ぶ
- **THEN** `cache_fingerprint` は異なる
- **THEN** dark mode の出力は PlantUML 公式の `ColorMapper.DARK_MODE` に由来する
- **THEN** class icon と visibility icon は PlantUML 公式の意味表現を保持する

#### Scenario: 公式 theme を切り替える

- **GIVEN** 同一 PlantUML source と `plantuml_theme=cyborg`
- **WHEN** PlantUML renderer を呼ぶ
- **THEN** 描画 source には KDR 独自の色上書きではなく PlantUML の `!theme cyborg` 相当が渡される
- **THEN** `cache_fingerprint` は theme 未指定時と異なる

#### Scenario: CLI help と API で theme 候補を確認できる

- **GIVEN** 利用者が `kdr plantuml render --help` を表示する
- **WHEN** help text を確認する
- **THEN** `--theme` の説明には指定可能な theme 名として `cyborg`、`black-knight`、`spacelab` が含まれる
- **THEN** `--theme-mode` の候補として `dark` と `light` が含まれる
- **WHEN** API 利用者が `PlantUmlRenderer::available_themes()` または `PlantUmlThemeCatalog::names()` を呼ぶ
- **THEN** CLI help と同じ theme 名一覧を取得できる

#### Scenario: KatanA のプレビュー背景で読める SVG を返す

- **GIVEN** KatanA dark theme の `RenderInput.context.theme`
- **WHEN** sequence / class / activity の PlantUML fixture を描画する
- **THEN** SVG は PlantUML 公式 dark mode の背景、文字、線、矢印で描画される
- **THEN** class fixture は class / abstract class / interface / enum / note / 関係線 / 属性 / メソッドを含む
- **THEN** class icon や visibility icon は PlantUML 公式の意味色を残す
- **THEN** 配色制御は SVG 後加工ではなく PlantUML の theme / dark mode 指定で行う
