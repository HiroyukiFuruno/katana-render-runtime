## Why

Issue [#9](https://github.com/HiroyukiFuruno/katana-diagram-renderer/issues/9) は、KDV の HTML 書き出しで `plantuml` ブロックが raw code のまま残る問題を報告している。
KDR は KatanA から外部図形描画を切り出した repository なので、PlantUML の SVG 描画責務も KDR が担うべきである。

## What Changes

- `spike/plantuml-jvm-embedding` で得た JVM 埋め込み検証結果を実装前提として OpenSpec に固定する
- KDR の公開 API（public API: 外部から呼ぶ約束）で `DiagramKind::PlantUml` を受け取り、`PlantUmlRenderer` から SVG または raw code string を返せるようにする
- `kdr plantuml render` / `reference-update` / `compare` / `bench` を追加し、`libjvm` や `plantuml.jar` が無い場合は警告（warning）をログへ明示し、raw code block を返す
- CLI（command line interface: コマンドで使う入口）と公開 API（public API: 外部から呼ぶ約束）は、同じ警告情報で「何が起きたか」と「利用者が何をすべきか」を返す
- sequence / class / activity の最小 PlantUML fixture を追加し、SVG 生成と metadata 抽出を検証する
- 初期実装は Rust から JVM（Java Virtual Machine: Java 仮想マシン）を起動する経路を第一候補にし、子 process 実行は JVM 埋め込みを採らない場合の明示的な代替案にする
- Java 実行環境（Java runtime）は同梱しないが、macOS / Linux / Windows の `libjvm` 検出、上書き指定、早期 return 診断を KDR が持つ
- `plantuml.jar` は KDR の runtime asset として version / checksum / 更新手順を固定し、実行時 network 取得に依存しない
- PlantUML SVG は Mermaid / Draw.io と同じ theme 契約に寄せ、背景、文字、線、矢印、participant、note、class の色を KatanA 表示で使える状態にする
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
