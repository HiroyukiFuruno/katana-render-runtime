# katana-canvas-forge OpenSpec

## Project

`katana-canvas-forge`（kcf）は、Mermaid / Draw.io 描画と HTML / PDF / PNG / JPEG export を担う versioned rendering library。KatanA はこれを git dependency として consume するだけ。描画実装・版管理・採点運用はすべてここで行う。

## Design Principles

- KatanA の public interface に kcf 固有の型を漏らさない。KatanA が見るのは `Renderer` trait と中立 DTO のみ。
- `egui` / KatanA UI state に依存しない。将来 KatanA が egui から脱却しても kcf は無影響。
- CLI（`kcf`）は library の薄い利用者として設計する。

## Versioning

- `v0.1.x`: Mermaid runtime interface 確定 + Mermaid backend 移管 + kcf CLI
- `v0.2.x`: Draw.io backend
- `v0.3.x`: HTML / PDF / PNG / JPEG export
- `v0.4.x`: 公式比較画像・採点・CI 運用

## Consumers

- [KatanA](https://github.com/HiroyukiFuruno/KatanA) — git tag pinned dependency
