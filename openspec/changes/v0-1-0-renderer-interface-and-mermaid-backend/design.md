## Context

v0.1.0 の目的は、KatanA 既存の rendering/export runtime と検証責務を kcf に忠実移植すること。新規に簡略実装を作る作業ではない。

PR #1 は close 済み。実装差分は土台にしない。レビュー履歴は、避けるべき方針と検証漏れの参照資料として使う。

## Goals

- KatanA の Mermaid 実装を kcf へ移植する
- KatanA の Draw.io 実装と resource 一式を kcf へ移植する
- KatanA の HTML / PDF / PNG / JPEG export 実装を kcf へ移植する
- KatanA の Mermaid / Draw.io reference 生成と ImageMagick 採点評価を kcf へ移植する
- KatanA 側でできていた Mermaid / Draw.io / export / score の能力を落とさない kcf 公開 API を提供する
- kcf は KatanA UI state に依存しない library と CLI を提供する

## Non-Goals

- PR #1 の簡略 Mermaid 実装を育てない
- KatanA 側でできていた機能を kcf 独自都合で縮小、改名、簡略化しない
- HTML のみ export を v0.1.0 完了扱いにしない
- SVG 文字列比較だけの score を採点評価として採用しない
- Draw.io / Mermaid の score 改善は v0.1.1 に送る
- Mermaid.js / Draw.io.js の取り込み version 固定、最新版確認、取り込み just recipe は v0.1.2 に送る
- Mermaid ZenUML / unsupported fixture handling は v0.1.3 に送る
- 実表示 E2E（viewer e2e）は v0.1.4 に送る
- CSV / PDF / Word / Excel / PPTX viewer rendering は v0.1.0 に含めない
- Native backend 化や外部プロセス依存ゼロ化は v0.1.0 の目的にしない

## Source Of Truth

移植元は KatanA 側の既存実装を正本とする。

- Mermaid runtime: `crates/katana-core/src/markdown/mermaid_renderer/`
- Draw.io runtime: `crates/katana-core/src/markdown/drawio_renderer/`
- export runtime: `crates/katana-core/src/markdown/export/`
- Mermaid scoring: `scripts/mermaid/`
- Draw.io scoring: `scripts/drawio/`
- Mermaid fixtures: `assets/fixtures/mermaid_parts/`
- Draw.io fixtures: `assets/fixtures/drawio/`
- export tests: `crates/katana-core/tests/export_regression.rs`
- visual export tests: `crates/katana-core/tests/native_export_visual.rs`
- Draw.io tests: `crates/katana-core/tests/markdown_drawio*.rs`
- Mermaid tests: `crates/katana-core/tests/markdown_mermaid.rs` と `mermaid_js_runtime_*.rs`

## Boundary Design

kcf は KatanA を知らない library として設計する。

- `Renderer` / `Exporter` は kcf の独立 library として自然な公開 API にする
- KatanA 側で定義済みの interface と DTO は、既存能力が落ちないことを確認するための照合元にする
- `Renderer` は Mermaid / Draw.io の SVG とメタデータを返す
- `Exporter` は HTML / PDF / PNG / JPEG の出力 path と format を返す
- CLI は library の薄い利用者に留める
- KatanA UI state、preview state、workspace state は DTO に持ち込まない
- v0.1.0 では既存 runtime asset を動作可能な形で移す
- vendor bundle、checksum、version pinning、resource manifest の整理は v0.1.2 で kcf 管理に固定する
- KatanA 側は git tag pinned dependency と薄い adapter を持つ
- adapter は型名合わせだけに留め、KatanA 側でできていた描画、書き出し、採点評価の機能差分を埋める追加実装を必要としない

## Nullable Boundary And Error First

最表層であっても、未指定に仕様上の意味がない値は非 null として受ける。
`Option` を許すのは、CLI の `--runtime` のように「未指定なら同梱 runtime を解決する」という意味が入力契約に含まれる境界だけに限定する。

runtime path は `RuntimePathResolver` で冒頭に解決し、成功時は `PathBuf`、失敗時は `RenderError::RuntimeResolution` にする。
`MermaidRenderer` / `DrawioRenderer` などの内部 renderer は `PathBuf` を直接保持し、`Option<PathBuf>` や暗黙 fallback を持たない。

到達不能に近い分岐は、coverage を埋めるために残さない。
実際に発生し得る不正入力は型付きエラーとして冒頭で弾き、発生しない分岐は dead logic として削除する。

## Transfer Approach

移植は責務単位で行う。

1. KatanA 側の現行実装とテストを棚卸しする
2. KatanA 側で定義済みの interface と DTO を棚卸しし、kcf 公開 API で既存能力が落ちないことを照合する
3. Mermaid backend を移植し、既存 fixture と reference score を維持する
4. Draw.io backend と resource resolver を移植し、既存 fixture と reference score を維持する
5. HTML / PDF / PNG / JPEG export を移植し、既存回帰テストの検証観点を kcf 側で維持する
6. CLI、just、CI に reference-update / compare / bench / export を接続する
7. KatanA 側の consumer integration は KatanA 側の関心として分離し、kcf では adapter が必要とする能力を公開 API と検証で固定する

## KatanA Compatibility Boundary

kcf は KatanA のためだけの crate ではないため、KatanA 側の `diagram_backend` 型名をそのまま公開 API にすること自体は完了条件にしない。

完了条件は、KatanA 側の adapter が薄い変換だけで次の能力を失わずに利用できることである。

- Mermaid / Draw.io の入力 source と diagram kind を渡せる
- Mermaid / Draw.io の SVG、幅、高さ、viewBox、runtime version、profile、diagnostics、cache fingerprint を受け取れる
- runtime 未導入、runtime error、未対応 diagram kind を区別できる
- HTML / PDF / PNG / JPEG export を同じ export trait 経由で呼べる
- 未対応 format は暗黙 fallback ではなく `UnsupportedFormat` として扱える
- Mermaid / Draw.io の reference 生成、画像化、score compare を kcf 側で実行できる

KatanA 側の `DiagramBackendInput` / `DiagramBackendOutput` / `DiagramBackendError` / `DiagramThemeSnapshot` は、互換性確認の照合元として扱う。kcf の `RenderInput` / `RenderOutput` / `RenderError` / `RenderConfig` / `RenderPolicy` / `RenderContext` が同じ情報を保持できる場合、型名の完全一致は不要とする。

## KatanA Test Migration Boundary

KatanA 側のテストをそのまま残すのではなく、検証観点を kcf 側へ移す。

- Mermaid / Draw.io の runtime 解決、未導入、失敗経路、代表 diagram render は kcf の integration test で確認する
- Draw.io official fixture、Mermaid fixture、score floor は kcf の reference compare と full compare に寄せる
- export regression と native visual export は、KatanA の Markdown renderer ではなく kcf の `Exporter` 入力で確認する
- KatanA 固有の Markdown parser、preview state、workspace state、UI state の検証は kcf に持ち込まない

## Reference Artifact Ownership

kcf でも KatanA 本家と同じく、公式 reference の SVG / PNG は git 管理する。CI/CD は外部から公式描画結果を再取得・再生成しない。

- `reference-update` は開発者が明示的に実行する更新操作とする
- `compare` は git 管理済み reference と、その場で生成した kcf 出力を比較する
- CI/CD は `compare` のみを実行し、reference artifact を変更しない
- Mermaid.js / Draw.io.js の version 更新時だけ、v0.1.2 の update recipe で reference SVG / PNG を再生成する
- reference 更新後は差分を review し、SVG / PNG と checksum / manifest を同じ変更として扱う

## Reference Evaluation Tiers

採点評価は、実行場所ごとに責務を分ける。

- 疎通確認（smoke check）: `basic` fixture だけを使う。描画処理が起動することを確認する用途であり、vendor 互換性の保証には使わない
- 代表ケース評価（representative evaluation）: 継続的統合 / 継続的配信（CI/CD）で使う。主要な図種、画像、HTML label、layer、cloud stencil、UML、network、floor plan を少数ずつ固定する
- 全量評価（full evaluation）: ローカルの release validation で使う。KatanA から移植した Mermaid / Draw.io 公式 fixture と git 管理済み reference artifact を全て比較する

CI/CD は実行時間と安定性を優先し、代表ケース評価だけを必須にする。全量評価は release branch の手元確認、runtime 更新、score 改善、疑わしい差分の調査で実行する。

Draw.io の `basic` は単純図形だけなので、vendor 互換性の代理指標にしてはならない。vendor 互換性は `official` full fixture と `representative` fixture の両方で確認する。

Draw.io representative で現時点の移植実装が 99 点に届かない case は、`score-baseline.json` に既知下限として明示する。これは合格基準の隠蔽ではなく、現在値からの悪化を CI/CD で検知するための下限である。v0.1.0 release 時点では KatanA 側へまだ取り込まないため既存品質は劣化しない。KatanA 側へ取り込む前に、score 改善で下限を上げる作業は v0.1.1 に送る。

## Coverage Gate

v0.1.0 は行カバレッジ（line coverage）100%、未到達行（uncovered line）0 を merge 条件にする。
`just coverage` は `cargo llvm-cov --workspace --all-features --locked --summary-only --fail-under-lines 100 --fail-uncovered-lines 0` を実行する。

未検証の分岐を残して coverage 下限だけを下げることはしない。
本当に必要なロジックは unit test / integration test で検証し、不要な fallback や dead logic は削除する。

## PR #1 Reference Policy

PR #1 は実装資産として使わない。

- close 済み PR として履歴を残す
- branch は必要に応じて参照する
- 指摘済みの失敗例を v0.1.0 の検証観点へ反映する
- PR #1 の差分を merge / cherry-pick しない

## Version Roadmap

transfer が完了した後、次の versioned change として扱う。

- `v0.1.1`: Draw.io / Mermaid の score 改善
- `v0.1.2`: Mermaid.js / Draw.io.js の version 固定、最新版確認、取り込み just recipe を整備する
- `v0.1.3`: Mermaid ZenUML / unsupported fixture handling を整備する
- `v0.1.4`: 実表示 E2E（viewer e2e）を追加する
- `v0.2.0`: CSV viewer rendering
- `v0.3.0`: PDF viewer rendering
- `v0.4.0`: Office viewer rendering。対象は Word / Excel / PPTX に限定する
- `v0.4.x`: バグ取りと継続的な score 向上
- `v0.5.0`: CLI 公開

viewer rendering は export とは別責務。export は「文書を外部ファイルへ書き出す」処理であり、viewer rendering は「既存ファイルを画面表示向けに描画する」処理である。

### v0.1.x Follow-up

`v0.1.4` の viewer e2e は、SVG / PNG / PDF を実ウィンドウで開いて目視確認、またはスクリーンショット確認するための E2E とする。`floem` / `egui` などの画面表示ライブラリは `crates/` 配下の通常 library / CLI 依存に入れず、`test/e2e/viewer/` などの E2E 専用領域に閉じ込める。core library は引き続き KatanA UI state、preview state、workspace state を持たない。自動採点の正本は ImageMagick score とし、viewer e2e は実表示の確認補助に限定する。
