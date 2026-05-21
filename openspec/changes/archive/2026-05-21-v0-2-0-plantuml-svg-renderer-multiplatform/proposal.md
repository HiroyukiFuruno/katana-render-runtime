## Why

Issue [#9](https://github.com/HiroyukiFuruno/katana-diagram-renderer/issues/9) は、KDV の HTML 書き出しで `plantuml` ブロックが raw code のまま残る問題を報告している。
KDR は KatanA から外部図形描画を切り出した repository なので、PlantUML の SVG 描画責務も KDR が担うべきである。

## What Changes

- `spike/plantuml-jvm-embedding` で得た JVM 埋め込み検証結果を実装前提として OpenSpec に固定する
- KDR の公開 API（public API: 外部から呼ぶ約束）で `DiagramKind::PlantUml` を受け取り、`PlantUmlRenderer` から SVG または raw code string を返せるようにする
- `kdr plantuml render` / `reference-update` / `compare` / `bench` を追加し、`libjvm` や `plantuml.jar` が無い場合は警告（warning）をログへ明示し、raw code block を返す
- `kdr plantuml render --theme --theme-from --theme-mode` と API の `vendor_config` で PlantUML 公式 theme と dark / light mode を切り替えられるようにする
- CLI（command line interface: コマンドで使う入口）の help は指定できる theme 名を表示し、公開 API（public API: 外部から呼ぶ約束）は同じ theme 名一覧を返せるようにする
- CLI（command line interface: コマンドで使う入口）と公開 API（public API: 外部から呼ぶ約束）は、同じ警告情報で「何が起きたか」と「利用者が何をすべきか」を返す
- sequence / use-case / class / object / activity / component / deployment / state / timing の公式 9 パターン fixture を追加し、Mermaid / Draw.io と同じ画像比較 score で 100 点を検証する
- 初期実装は Rust から JVM（Java Virtual Machine: Java 仮想マシン）を起動する経路を第一候補にし、子 process 実行は JVM 埋め込みを採らない場合の明示的な代替案にする
- Java 実行環境（Java runtime）は同梱しないが、macOS / Linux / Windows の `libjvm` 検出、上書き指定、早期 return 診断を KDR が持つ
- `plantuml.jar` は固定 URL / checksum / cache prefetch / update 手順を固定し、crate package には checksum manifest だけを含める
- PlantUML SVG は PlantUML 公式 dark mode と公式 theme 契約に寄せ、KDR 独自の大きな `skinparam` 上書きや SVG 後加工で公式表現を劣化させない。class icon と visibility icon は PlantUML 公式の意味色を残す
- 子 process 実行を採る場合のみ、Windows では端末窓を出さない process facade を使い、`cmd` / PowerShell wrapper に逃がさない
- KDV 側の責務は、KDR から受け取った SVG を `<figure data-kdv-diagram="plantuml">...<svg>...</svg>...</figure>` に埋め込むことに限定する

## Capabilities

### New Capabilities

- `plantuml-svg-rendering`: PlantUML source を KDR の renderer / CLI / reference 評価で SVG へ変換する契約
- `plantuml-runtime-dependency-management`: Java 実行環境と `plantuml.jar` を複数 OS（multi platform: macOS / Linux / Windows）で解決・診断・検証する契約

### Modified Capabilities

- `renderer-runtime-interface`: KDR の外部図形描画 API が Mermaid / Draw.io に加えて PlantUML を正式に扱う
- `runtime-asset-versioning`: Mermaid.js / Draw.io.js / ZenUML に加え、PlantUML JAR の version / checksum / 更新手順を固定する

## Impact

- `crates/katana-diagram-renderer/src/renderer/api.rs`
- `crates/katana-diagram-renderer/src/renderer/backends.rs`
- `crates/katana-diagram-renderer/src/renderer/runtime.rs`
- `crates/katana-diagram-renderer/src/renderer/runtime_path.rs`
- `crates/katana-diagram-renderer/src/markdown/plantuml_renderer/`
- `crates/katana-diagram-renderer-cli/src/{commands.rs,main.rs,diagram_cmd.rs,reference_cmd.rs}`
- `tests/fixtures/plantuml/**`
- `Justfile` の PlantUML render / reference / compare / bench / runtime asset recipe
- `.github/workflows/**` の macOS / Ubuntu / Windows PlantUML smoke check
- KDV 側の `kdr-plantuml` / `plantuml-render` gate を `ExternalBackendRequired` から外す後続作業
