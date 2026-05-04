# katana-canvas-forge OpenSpec

## Project

`katana-canvas-forge`（kcf）は、Mermaid / Draw.io 描画と HTML / PDF / PNG / JPEG export を担う versioned rendering library。KatanA はこれを git dependency として consume するだけ。描画実装・版管理・採点運用はすべてここで行う。

## Design Principles

- KatanA の public interface に kcf 固有の型を漏らさない。KatanA が見るのは `Renderer` trait と中立 DTO のみ。
- `egui` / KatanA UI state に依存しない。将来 KatanA が egui から脱却しても kcf は無影響。
- CLI（`kcf`）は library の薄い利用者として設計する。

## Versioning

- `v0.1.x`: KatanA の現在の描画責務を一括で受け取る最初の MVP release
  - `Renderer` trait + 中立 DTO 確定
  - Mermaid backend（外部 mmdc 経由 / Rust 管理 JS）
  - Draw.io backend
  - HTML / PDF / PNG / JPEG export（`Exporter` trait）
  - kcf CLI（`render` / `reference-update` / `compare` / `bench`）
  - Mermaid.js 版固定（`vendor/mermaid/<version>/`）
- `v0.2.x`: Native backend 移行 — Node.js / Java 外部プロセス依存ゼロ化
  - Mermaid native backend（`merman` 等）
  - PlantUML native backend（`plantuml-little` 等）
  - Draw.io native export
- `v0.3.x`: 公式比較画像・採点・CI 運用の本格化

> **方針**: KatanA `release/v0.22.10` 時点で同一実装内に密結合している Mermaid + Draw.io + export を、kcf v0.1.0 で一括引き受けする。中途半端に分割すると KatanA 側で hybrid 状態（一部 kcf、一部 KatanA 内）が発生し、結合検証コストが増える。一括移管が現実的。

## Consumers

- [KatanA](https://github.com/HiroyukiFuruno/KatanA) — git tag pinned dependency

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

### egui から脱却する理由（要約）

- カラー絵文字：epaint が SBIX/CBTF 非対応 → cosmic-text で解決
- IME 不完全：egui TextEdit の composition が壊れる → cosmic-text + winit で解決
- レイアウト拡張不可：vendor パッチなしに行間・マージンを変えられない → vello Scene への直接描画で解決
- immediate mode の再描画コスト → vello の retained 描画で解決

### この repo の責務

各 `-egui` impl crate を `-floem` impl crate に差し替える。neutral interface crate は変えない。
KatanA の `Cargo.toml` の impl crate 行を変えるだけで移行が完了する。

### katana-canvas-forge の移行

kcf は最初から egui 非依存（neutral only）のため、UI フレームワーク移行の影響を受けない。
wgpu を描画バックエンドとして直接使うことは将来的に検討できるが、現時点では変更不要。
