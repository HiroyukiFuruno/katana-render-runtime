# katana-canvas-forge OpenSpec

## Project

`katana-canvas-forge`（KCF）は、Mermaid / Draw.io / PlantUML / mathなどの外部描画を担うversioned rendering library。KatanA はこれを git dependency としてconsumeするだけ。描画実装・版管理・採点運用はここで行う。

既存HTML / PDF / PNG / JPEG exportは、`katana-document-viewer`（KDV）に同等機能が入るまで維持する。KDV実装後、export関連計画と実装はKDVへ移譲し、KCF側から削除する。

## Design Principles

- KatanA の public interface に kcf 固有の型を漏らさない。KatanA が見るのは `Renderer` trait と中立 DTO のみ。
- v0.1.0 transfer では KatanA 側で定義済みの interface を正本として完全踏襲する。KDV移譲までは既存exportを維持する。
- 新規export責務はKCFに追加しない。Markdown viewer/exportはKDV側で扱う。
- `egui` / KatanA UI state に依存しない。将来 KatanA が egui から脱却しても kcf は無影響。
- CLI（`kcf`）は library の薄い利用者として設計する。

## Versioning

- `v0.1.0`: KatanA 既存 rendering/export runtime の忠実移植（transfer）。exportはKDV移譲までの維持対象。
  - `Renderer` / `Exporter` trait + 中立 DTO 確定
  - KatanA 側で定義済みの interface を完全踏襲
  - KatanA 既存 Mermaid backend の移植
  - KatanA 既存 Draw.io backend と resource 一式の移植
  - KatanA 既存 HTML / PDF / PNG / JPEG export の移植
  - Mermaid / Draw.io の公式 reference 生成と ImageMagick 採点評価の移植
  - 開発用 kcf CLI（`render` / `reference-update` / `compare` / `bench`）
  - fixture、既存 test、CI の移植
  - runtime asset の取得 version 固定や更新 recipe 改善は `v0.1.1` に送る
  - full score 未達は KatanA 側へ取り込む前の最終段階として `v0.1.4` で改善する
- `v0.1.1`: Mermaid.js / Draw.io.js runtime asset version pinning
  - Mermaid.js と Draw.io.js の取り込み version を kcf で固定する
  - 現在取り込める最新版を確認する just recipe を追加する
  - 指定 version を取り込み、checksum、manifest、reference snapshot を更新する just recipe を追加する
  - v0.1.0 transfer で露出した runtime asset 管理の課題を小規模 patch として解決する
- `v0.1.2`: Mermaid ZenUML / unsupported fixture handling
  - `28-zen-uml.md` の supported / unsupported 境界を固定する
  - unsupported fixture は暗黙 skip せず、理由を report に残す
  - compare が空出力や未生成 PNG で null 参照しないようにする
- `v0.1.3`: RenderInput theme application
  - consumer が渡す theme snapshot を Mermaid / Draw.io の実描画に反映する
  - cache fingerprint は process global state ではなく、RenderInput 由来の実効 theme を使う
  - global theme state は後方互換 fallback に限定する
- `v0.1.4`: reference score improvement
  - Jules 側で停滞している旧 v0.1.1 の範囲を v0.1.x の最後に回す
  - Draw.io official / representative の既知 score 未達を改善する
  - score baseline は下げず、修正後の下限として上げる
  - Mermaid supported fixture の score 回帰を確認する
- `v0.1.5`: KDV export handoff
  - 不要になった export CSS debug 計画はKCF release番号から外す
  - 旧 export/debug 論点はKDV側へ移譲する
  - KCF側は Mermaid / Draw.io rendering、runtime asset、reference score に責務を絞る
- `v0.2.0`: KDV移譲計画へ変更
  - CSV / PDF / Office viewer renderingはKDV側へ移す
  - KCF側で必要な外部描画APIだけを残す
- `v0.3.0`: KDV移譲計画へ変更
  - PDF viewer renderingはKDV側へ移す
  - KCF側では外部描画referenceとscoreを維持する
- `v0.4.0`: KDV移譲計画へ変更
  - Office viewer renderingはKDV側へ移す
  - KCF側ではOffice viewer責務を持たない
- `v0.4.x`: バグ取りと score 向上
  - Mermaid / Draw.io / 既存export保守 / 外部描画reference のバグ修正
  - v0.1.4 後に残る reference score、baseline policy、fixture coverage、差分 report の継続改善
- `v0.5.0`: CLI 公開
  - 外部利用者向けの CLI surface、help、exit code、配布、release 手順を固定する
  - 開発用 CLI と公開 CLI の差分を整理する

> **方針**: KatanA `release/v0.22.10` 時点で同一実装内に密結合している Mermaid + Draw.io + export + 採点評価を、KCF v0.1.0 で一括引き受けした。既存exportはKDVに同等機能が入るまで維持するが、新規export計画はKDVへ移す。KCFの最終責務は外部描画、runtime asset管理、reference scoreである。

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
