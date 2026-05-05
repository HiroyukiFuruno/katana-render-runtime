## ADDED Requirements

### Requirement: 実表示 E2E を core library と CLI から隔離しなければならない

システムは、実表示 E2E（viewer e2e）を `test/e2e/viewer/` などの E2E 専用領域に隔離しなければならない（MUST）。`crates/katana-canvas-forge` と `crates/katana-canvas-forge-cli` は、`floem` / `egui` などの画面表示依存を通常依存として持ってはならない（MUST NOT）。

#### Scenario: workspace dependency を確認する

- **WHEN** `cargo tree --workspace -e normal` を実行する
- **THEN** `crates/katana-canvas-forge` と `crates/katana-canvas-forge-cli` の通常依存に `floem` / `egui` が含まれない
- **THEN** KatanA UI state、preview state、workspace state が kcf core library に入らない

#### Scenario: viewer e2e を build する

- **WHEN** 実表示 E2E を build する
- **THEN** viewer e2e は `test/e2e/viewer/` の専用 entrypoint から build される
- **THEN** workspace の通常 test、publish package、core library API に画面表示依存が混ざらない

### Requirement: viewer e2e は生成済み artifact を入力にしなければならない

viewer e2e は、kcf の renderer / exporter を直接呼ぶのではなく、事前に生成された SVG / PNG / JPEG / PDF / HTML と reference artifact を入力にしなければならない（MUST）。

#### Scenario: Mermaid 出力を表示する

- **WHEN** viewer e2e が Mermaid case を開く
- **THEN** 生成済み reference artifact と kcf artifact を読み込む
- **THEN** renderer 実装を viewer e2e 内で再実装しない

#### Scenario: Draw.io 出力を表示する

- **WHEN** viewer e2e が Draw.io case を開く
- **THEN** 生成済み reference artifact と kcf artifact を読み込む
- **THEN** Draw.io runtime を viewer e2e 内で直接初期化しない
