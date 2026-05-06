# katana-canvas-forge OpenSpec

## Project

`katana-canvas-forge`（kcf）は、Mermaid / Draw.io 描画と HTML / PDF / PNG / JPEG export を担う versioned rendering library。KatanA はこれを git dependency として consume するだけ。描画実装・版管理・採点運用はすべてここで行う。

## Design Principles

- KatanA の public interface に kcf 固有の型を漏らさない。KatanA が見るのは `Renderer` trait と中立 DTO のみ。
- v0.1.0 transfer では KatanA 側で定義済みの interface を正本として完全踏襲する。kcf 側で独自に縮小、改名、簡略化しない。
- `egui` / KatanA UI state に依存しない。将来 KatanA が egui から脱却しても kcf は無影響。
- CLI（`kcf`）は library の薄い利用者として設計する。

## Versioning

- `v0.1.0`: KatanA 既存 rendering/export runtime の忠実移植（transfer）
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
- `v0.1.2`: export CSS regression fix / macOS debug open
  - PDF / PNG / JPEG export に HTML の `body` 向けCSSを反映する
  - export 4形式を `/tmp` に出力し、macOS の既定アプリで開く debug command を追加する
- `v0.1.3`: Mermaid ZenUML / unsupported fixture handling
  - `28-zen-uml.md` の supported / unsupported 境界を固定する
  - unsupported fixture は暗黙 skip せず、理由を report に残す
  - compare が空出力や未生成 PNG で null 参照しないようにする
- `v0.1.4`: reference score improvement
  - Jules 側で停滞している旧 v0.1.1 の範囲を v0.1.x の最後に回す
  - Draw.io official / representative の既知 score 未達を改善する
  - score baseline は下げず、修正後の下限として上げる
  - Mermaid supported fixture の score 回帰を確認する
- `v0.2.0`: CSV viewer rendering
  - CSV を構造化して render し、viewer に渡せる形式へ変換する
  - 表形式、列幅、型推定、文字コード、巨大 CSV の扱いを仕様化する
- `v0.3.0`: PDF viewer rendering
  - PDF ファイルを viewer 用に render する
  - page navigation、scale、text layer、thumbnail、cache の扱いを仕様化する
- `v0.4.0`: Office viewer rendering
  - Word / Excel / PPTX に限定して viewer 用に render する
  - Office 系の対象 format はこの 3 種に限定し、他の Office format は別 change にしない限り扱わない
- `v0.4.x`: バグ取りと score 向上
  - Mermaid / Draw.io / export / viewer rendering のバグ修正
  - v0.1.4 後に残る reference score、baseline policy、fixture coverage、差分 report の継続改善
- `v0.5.0`: CLI 公開
  - 外部利用者向けの CLI surface、help、exit code、配布、release 手順を固定する
  - 開発用 CLI と公開 CLI の差分を整理する

> **方針**: KatanA `release/v0.22.10` 時点で同一実装内に密結合している Mermaid + Draw.io + export + 採点評価を、kcf v0.1.0 で一括引き受けする。新規に簡略版を作るのではなく、KatanA 既存実装を正本として移植し、KatanA 固有 UI state と path 前提だけを剥がす。v0.1.0 は KatanA 側へまだ取り込まないため release 可能とする。旧 v0.1.1 の score 改善（score improvement）は Jules 側で停滞しているため v0.1.x の最後へ回し、実行時資産のバージョン固定（runtime asset version pinning）と更新 recipe を v0.1.1、export CSS 回帰修正と macOS debug open を v0.1.2、ZenUML / unsupported fixture handling を v0.1.3、score 未達改善を v0.1.4 に分離する。CSV / PDF / Office viewer rendering と CLI 公開は、その後に進める。

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
