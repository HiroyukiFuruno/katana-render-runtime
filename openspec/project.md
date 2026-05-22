# katana-render-runtime OpenSpec

## Project

`katana-render-runtime`（KRR）は、Mermaid / Draw.io / ZenUML / PlantUML / MathJax を使い、入力文字列を SVG などの描画成果物へ変換する versioned rendering library である。
`katana-diagram-renderer`（KDR）は `v0.3.0` から互換 wrapper として残す。

HTML / PDF / PNG / JPEG などの文書出力は KRR の責務外とし、KRR は外部 runtime、asset 管理、SVG 出力、diagnostics、reference score に集中する。

## Design Principles

- KatanA / KDV の public interface に KRR 固有の型を不用意に漏らさない。上位 consumer が見るのは中立 DTO と描画成果物である。
- Markdown AST の解析、`$...$` / `$$...$$` / fenced `math` の判定、export surface 生成は KRR の責務にしない。
- 文書出力責務は KRR に追加しない。Markdown viewer / export は KRR 外で扱う。
- `egui` / KatanA UI state に依存しない。将来 KatanA が UI framework を変更しても KRR は無影響である。
- CLI（`kdr`）は library の薄い利用者として設計する。
- 旧 crate 名は migration note と compatibility wrapper の説明に限定する。

## Versioning

- `v0.1.0`: KatanA 既存 rendering runtime の忠実移植（transfer）。
  - Mermaid / Draw.io runtime、reference 生成、ImageMagick 採点評価、開発用 `kdr` CLI を移植する。
- `v0.1.1`: Mermaid.js / Draw.io.js runtime asset version pinning。
  - runtime asset の version、checksum、更新 recipe を固定する。
- `v0.1.2`: Mermaid ZenUML / unsupported fixture handling。
  - supported / unsupported 境界と compare report を固定する。
- `v0.1.3`: RenderInput theme application。
  - consumer が渡す theme snapshot を Mermaid / Draw.io の実描画に反映する。
- `v0.1.4`: ZenUML foreignObject fix（緊急）。
  - ZenUML 出力を native image path でも非空・非白にする。
- `v0.1.5`: reference score improvement。
  - Draw.io official / representative の既知 score 未達を改善する。
- `v0.1.6`: KDV export handoff。
  - export/debug 論点は KDV 側へ移譲し、KDR 側は runtime asset と reference score に責務を絞る。
- `v0.2.0`: PlantUML SVG renderer と multi platform runtime 対応。
  - PlantUML を SVG または diagnostics 付き raw code block として返す。
  - `plantuml.jar` は固定 URL / checksum で cache へ取得し、crates.io package には JAR 本体を含めない。
- `v0.3.0`: `katana-render-runtime` への rename と MathJax SVG runtime。
  - GitHub repository と primary crate を `katana-render-runtime` へ rename する。
  - `katana-render-runtime v0.3.0` を crates.io に公開する。
  - `katana-diagram-renderer v0.3.0` は `katana-render-runtime` の互換 wrapper として公開する。
  - MathJax v4 系で TeX input を SVG output へ変換する。
  - KRR は Markdown AST 解析を行わず、受け取った input string を SVG 化するだけにする。
  - SVG 化失敗時は diagnostics 付き raw string を返す。
  - MathJax JavaScript asset は checksum 管理し、`just mathjax-latest` / `just mathjax-update` で扱う。
- `v0.3.x`: バグ取りと score 向上。
  - Mermaid / Draw.io / PlantUML / MathJax の runtime contract、reference score、fixture coverage を継続改善する。
- `v0.4.0`: CLI 公開 surface 整理。
  - 外部利用者向けの CLI surface、help、exit code、配布、release 手順を固定する。

> **方針**: KRR の責務は外部 runtime、asset 管理、SVG 出力、diagnostics、reference score である。文書出力や viewer は KRR 外で扱う。

## Consumers

- [KatanA](https://github.com/HiroyukiFuruno/KatanA) — tag pinned dependency
- KDV — `katana-render-runtime v0.3.0` 公開後に dependency switch 可能

---

## UI フレームワーク移行方針（egui → Floem）

このセクションはエコシステム全体で共通の方針。詳細は [KatanA openspec/project.md](https://github.com/HiroyukiFuruno/KatanA/blob/master/openspec/project.md) を正とする。

### 技術選定（確定）

| 層 | 採用 |
|----|------|
| UI フレームワーク | **Floem**（Rust 純正・クロスプラットフォーム） |
| 文字描画 | **cosmic-text**（IME 完全対応・カラー絵文字 SBIX/CBTF） |
| 2D レンダリング | **vello + wgpu**（compute-shader・Metal/DX12/Vulkan） |
| レイアウト | **taffy**（flexbox + CSS Grid） |
| アーキテクチャ参考 | **GPUI / Zed**（設計の教材として活用） |

React / TypeScript / WebView は使用しない。Rust 純正のみ。

### この repo の責務

KRR は UI フレームワークに依存しない neutral runtime である。
wgpu を描画バックエンドとして直接使うことは将来的に検討できるが、現時点では変更不要。
