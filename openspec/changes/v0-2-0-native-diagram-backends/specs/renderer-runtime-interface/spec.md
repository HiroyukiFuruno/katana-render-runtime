## MODIFIED Requirements

### Requirement: 描画 backend は外部プロセス依存ゼロの Rust ネイティブ実装を選択できなければならない

システムは、Mermaid / PlantUML / Draw.io 描画 backend に Node.js / Java 外部プロセス依存ゼロの Rust ネイティブ実装を選択できるようにしなければならない（MUST）。kcf は `Renderer` trait の実装を差し替えるだけで backend を切り替えられる。

#### Scenario: Mermaid を Rust ネイティブ backend で描画する

- **WHEN** kcf が Mermaid backend として Rust ネイティブ実装（`merman` 等）を選択する
- **THEN** Node.js プロセスを起動せずに Mermaid SVG を生成する
- **THEN** 公式 Mermaid.js 出力との互換性を採点評価で検証する

#### Scenario: PlantUML を Rust ネイティブ backend で描画する

- **WHEN** kcf が PlantUML backend として Rust ネイティブ実装（`plantuml-little` 等）を選択する
- **THEN** Java プロセスを起動せずに PlantUML SVG を生成する

#### Scenario: Draw.io export を外部プロセスなしで生成する

- **WHEN** kcf が Draw.io 図形を SVG / PNG / PDF として export する
- **THEN** `vendor/drawio/<version>/drawio.min.js` + `.sha256` の固定版を使う
- **THEN** OS Chrome / Chromium app プロセスを起動しない
