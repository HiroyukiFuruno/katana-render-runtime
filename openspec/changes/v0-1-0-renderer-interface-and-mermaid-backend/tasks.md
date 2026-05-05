# Tasks: katana-canvas-forge v0.1.0 transfer

## Branch Rule

- **標準ブランチ**: `release/v0.1.0`
- **作業ブランチ**: `feature/v0.1.0-task-x`

---

## 0. Planning Baseline

### 目的

v0.1.0 を KatanA 既存実装の忠実移植として固定し、PR #1 の簡略実装へ戻らないようにする。

### 書き込み範囲

- `openspec/`
- GitHub PR #1 の参照履歴

### タスク

- [x] 0.1 PR #1 を close し、実装差分は土台にしない方針を明確化する
- [ ] 0.2 PR #1 のレビュー指摘を、v0.1.0 実装時の失敗パターンとして確認する
- [ ] 0.3 v0.1.0 の完了条件を transfer として再確認する

### Definition of Done

- [ ] PR #1 の差分を merge / cherry-pick しないことが確認されている
- [ ] v0.1.0 が Mermaid / Draw.io / export / score の移植であることが明文化されている
- [ ] extension 要件が v0.1.0 から分離されている

---

## 1. 移植元棚卸しと責務境界を固定する

### 目的

KatanA 側の移植元を漏れなく洗い出し、kcf 側の crate / module / CLI / vendor 配置へ写像する。

### 書き込み範囲

- `openspec/`
- `crates/katana-canvas-forge/src/`
- `crates/katana-canvas-forge-cli/src/`
- `vendor/`
- `tests/`

### タスク

- [ ] 1.1 `crates/katana-core/src/markdown/mermaid_renderer/` の移植対象を一覧化する
- [ ] 1.2 `crates/katana-core/src/markdown/drawio_renderer/` と resource 一式の移植対象を一覧化する
- [ ] 1.3 `crates/katana-core/src/markdown/export/` の HTML / PDF / PNG / JPEG export 対象を一覧化する
- [ ] 1.4 `scripts/mermaid/` と `scripts/drawio/` の reference 生成・採点対象を一覧化する
- [ ] 1.5 `assets/fixtures/mermaid_parts/` と `assets/fixtures/drawio/` の移植方針を決める
- [ ] 1.6 KatanA 側で定義済みの renderer / exporter interface と DTO を一覧化する
- [ ] 1.7 KatanA UI state、preview state、workspace state を kcf に持ち込まない境界を確認する

### Definition of Done

- [ ] 移植対象と非移植対象が artifact に残っている
- [ ] KatanA 固有依存を剥がす箇所が明確である
- [ ] v0.1.0 の作業範囲に extension 要件が混ざっていない

---

## 2. KatanA interface と DTO を完全踏襲する

### 目的

KatanA 側で定義済みの `Renderer` / `Exporter` contract を kcf の公開 API として踏襲する。

### 書き込み範囲

- `crates/katana-canvas-forge/src/renderer.rs`
- `crates/katana-canvas-forge/src/exporter.rs`
- `crates/katana-canvas-forge/src/lib.rs`
- `tests/`

### タスク

- [ ] 2.1 KatanA 側で定義済みの `Renderer` trait と DTO を kcf に移す
- [ ] 2.2 KatanA 側で定義済みの `Exporter` trait と DTO を kcf に移す
- [ ] 2.3 interface 名、field、format、error semantics を KatanA 側と照合する
- [ ] 2.4 Mermaid / Draw.io / export の失敗経路を KatanA 既存挙動と同等に表現する
- [ ] 2.5 KatanA consumer adapter で必要な情報が DTO にそろっているか確認する
- [ ] 2.6 mock 実装による trait 単体テストを追加する

### Definition of Done

- [ ] 公開 API が KatanA 側で定義済みの interface を踏襲している
- [ ] kcf 独自都合で field や format を削っていない
- [ ] 公開 API が KatanA UI 固有型を含まない
- [ ] `cargo tree --workspace -e normal` に `egui` と KatanA UI state が含まれない
- [ ] CLI が DTO を再定義していない

---

## 3. Mermaid backend を移植する

### Definition of Ready

- [ ] Task 1 と Task 2 が完了している
- [ ] Mermaid runtime asset を v0.1.0 でどう移すかが決まっている

### 目的

KatanA 既存 Mermaid runtime と fixture coverage を kcf へ移す。

### 書き込み範囲

- `crates/katana-canvas-forge/src/mermaid/`
- `vendor/mermaid/`
- `tests/fixtures/mermaid/`
- `tests/`

### タスク

- [ ] 3.1 KatanA Mermaid runtime script、DOM shim、normalizer、theme、i18n 処理を移植する
- [ ] 3.2 Mermaid runtime asset を KatanA 相当の読み込み形態で移す
- [ ] 3.3 Mermaid fixture と snapshot を KatanA から移す
- [ ] 3.4 `RenderOutput` の SVG、width、height、viewBox、runtime、diagnostics、cache fingerprint を実出力から生成する
- [ ] 3.5 Mermaid backend の unit test と integration test を移植する

### Definition of Done

- [ ] PR #1 の簡略 SVG 生成ではなく、KatanA 既存 runtime と同等の出力である
- [ ] version 固定と更新 recipe 改善は v0.1.1 に送られている
- [ ] Mermaid 既存 fixture が kcf 側で検証されている

---

## 4. Draw.io backend と resource 一式を移植する

### Definition of Ready

- [ ] Task 1 と Task 2 が完了している
- [ ] Draw.io runtime asset と resource を v0.1.0 でどう移すかが決まっている

### 目的

KatanA 既存 Draw.io runtime と resource resolver を kcf へ移す。

### 書き込み範囲

- `crates/katana-canvas-forge/src/drawio/`
- `vendor/drawio/`
- `tests/fixtures/drawio/`
- `tests/`

### タスク

- [ ] 4.1 Draw.io runtime adapter、mx adapter、SVG postprocess、resource resolver を移植する
- [ ] 4.2 Draw.io resource、stencil、image、manifest を kcf 管理へ移す
- [ ] 4.3 Draw.io runtime asset、resource、manifest を KatanA 相当の読み込み形態で移す
- [ ] 4.4 Draw.io fixture と official reference を KatanA から移す
- [ ] 4.5 Draw.io backend の unit test と integration test を移植する

### Definition of Done

- [ ] KatanA 既存 Draw.io tests と同等の検証が kcf 側にある
- [ ] resource 解決が kcf 内で完結している
- [ ] KatanA UI state に依存していない

---

## 5. HTML / PDF / PNG / JPEG export を移植する

### Definition of Ready

- [ ] Task 2 が完了している
- [ ] export input が HTML source と rendered SVG のどちらを受けるかが設計で固定されている

### 目的

KatanA 既存 export runtime を kcf へ移し、HTML だけで完了扱いにしない。

### 書き込み範囲

- `crates/katana-canvas-forge/src/exporter/`
- `tests/`

### タスク

- [ ] 5.1 HTML export と template 処理を移植する
- [ ] 5.2 PDF export を移植する
- [ ] 5.3 PNG export を移植する
- [ ] 5.4 JPEG export を移植する
- [ ] 5.5 export regression test と native visual export test を移植する

### Definition of Done

- [ ] HTML / PDF / PNG / JPEG が `Exporter` 経由で出力できる
- [ ] 未対応 format のみ `UnsupportedFormat` を返す
- [ ] export は KatanA の Markdown renderer や UI state に密結合していない

---

## 6. Mermaid / Draw.io 採点評価を移植する

### Definition of Ready

- [ ] Task 3 と Task 4 が完了している
- [ ] ImageMagick の実行前提と CI install 方法が決まっている

### 目的

KatanA 既存の公式 reference 生成と画像 score を kcf に移す。

### 書き込み範囲

- `crates/katana-canvas-forge-cli/src/`
- `tests/reference/`
- `scripts/` または kcf CLI 内部 module
- `.github/workflows/`

### タスク

- [ ] 6.1 Mermaid official reference 生成を移植する
- [ ] 6.2 Mermaid ImageMagick score と baseline policy を移植する
- [ ] 6.3 Draw.io official reference 生成を移植する
- [ ] 6.4 Draw.io ImageMagick score、PHASH、dimension coverage を移植する
- [ ] 6.5 contact sheet と report 生成を移植する
- [ ] 6.6 SVG 文字列比較だけでは合格できない検証を追加する

### Definition of Done

- [ ] `kcf mermaid compare --min-score <score>` が画像 score を出力する
- [ ] `kcf drawio compare --min-score <score>` が画像 score を出力する
- [ ] reference-update と compare が同じ fixture 管理を使う

---

## 7. CLI / just / CI を移植運用に接続する

### Definition of Ready

- [ ] Task 3 から Task 6 が完了している

### 目的

kcf CLI を library の薄い利用者として整え、移植後の品質ゲートを自動化する。

### 書き込み範囲

- `crates/katana-canvas-forge-cli/src/`
- `Justfile`
- `.github/workflows/`
- `lefthook.yml`

### タスク

- [ ] 7.1 `kcf mermaid render / reference-update / compare / bench` を接続する
- [ ] 7.2 `kcf drawio render / reference-update / compare / bench` を接続する
- [ ] 7.3 `kcf export html / pdf / png / jpeg` を接続する
- [ ] 7.4 just recipe を render、reference-update、compare、bench、export に接続する
- [ ] 7.5 CI で fmt / clippy / test / reference score / dependency leak check を実行する
- [ ] 7.6 lefthook または equivalent で軽量な pre-commit gate を設定する

### Definition of Done

- [ ] CLI が KatanA 固有 path を前提にしない
- [ ] runtime asset version 固定と更新 recipe 改善は v0.1.1 に送られている
- [ ] CI が PR #1 型の簡略実装を検知できる

---

## 8. KatanA consumer integration を確認する

### Definition of Ready

- [ ] Task 7 が完了している

### 目的

KatanA が kcf を git dependency として利用でき、hybrid 状態を残さないことを確認する。

### 書き込み範囲

- kcf 側 integration fixture
- KatanA 側 adapter 作業用 branch

### タスク

- [ ] 8.1 KatanA 側で kcf の `Renderer` / `Exporter` を consume する adapter 方針を確認する
- [ ] 8.2 KatanA 側に Mermaid / Draw.io / export の二重実装が残らない移行順序を決める
- [ ] 8.3 KatanA 側の既存 tests を kcf 利用状態で実行する
- [ ] 8.4 release tag `v0.1.0` で KatanA がビルドできることを確認する

### Definition of Done

- [ ] KatanA は kcf を git tag pinned dependency として利用できる
- [ ] KatanA 側に同じ描画 runtime の active 実装が重複して残っていない
- [ ] KatanA 側の user-facing behavior が維持されている

---

## 9. User Review

> ユーザーから受けた指摘は `[/]` で閉じる。通常の開発タスク `[x]` と混ぜない。

- [ ] 9.1 実装結果と検証結果をユーザーに提示する
- [ ] 9.2 Mermaid / Draw.io / export / score の移植完了証跡を提示する
- [ ] 9.3 フィードバックを本 `tasks.md` に追記し、対応済みを `[/]` にする

---

## 10. Final Verification

- [ ] 10.1 `/lint-and-ast-lint` を実行し、静的検査と抽象構文木検査の結果を記録する
- [ ] 10.2 `/self-review` を実行し、差分範囲の設計、テスト、検証の妥当性を確認する
- [ ] 10.3 `npx -y @fission-ai/openspec validate "v0-1-0-renderer-interface-and-mermaid-backend" --strict` を実行する
- [ ] 10.4 PR 作成が必要な場合は `/create_pull_request` を使う
- [ ] 10.5 統合後に `/openspec-archive-change` を実行する
