# Tasks: katana-canvas-forge v0.1.3 RenderInput theme application

## Branch Rule

- **標準ブランチ**: `release/v0.1.3`
- **作業ブランチ**: `feature/v0.1.3-task-x`

---

## User Feedback

- [/] Issue #4: KatanA から light を `RenderInput` に渡しても dark で描画される。kcf 側で `RenderInput` の theme 情報を実描画と cache fingerprint に反映する。
- [/] v0.1.3 の取り込み依頼（Pull Request）では full check を必須にし、merge 後の `master` では release だけを流し、`master` 直接 push は動作影響差分だけ Rust full check を実行する。
- [/] 完了済み OpenSpec change は release PR 作成前に archive へ移動し、archive 漏れを `lefthook run pre-pr` で検出する。
- [/] theme 経路だけの変更なので、score 評価や全fixture画像比較は実施しない。検証は theme / request / fingerprint の対象テストと標準品質ゲートに限定する。

---

## 1. 公開 DTO への typed theme snapshot 追加

### 目的

`RenderContext` に typed な theme snapshot を追加し、consumer が theme 情報を type-safe に渡せるようにする。

### タスク

- [x] 1.1 `RenderThemeMode` (Light / Dark) と `RenderThemeSnapshot` 型を `crates/katana-canvas-forge/src/renderer/api.rs` に追加する
- [x] 1.2 `RenderContext` に `theme: Option<RenderThemeSnapshot>` field を追加する（Default / Serialize / Deserialize / Clone を維持）
- [x] 1.3 `RenderThemeSnapshot` に含める field を `DiagramColorPreset` の描画影響 field に限定し、doc comment で対応関係を明示する
- [x] 1.4 `RenderContext.theme` と `theme_fingerprint` の優先順位を doc comment で明示する

### Definition of Done

- [x] 既存 consumer の build を壊さず、`RenderContext::default()` に `theme: None` が入る
- [x] `RenderThemeSnapshot` が `Hash` / `Clone` / `Serialize` / `Deserialize` を満たす

---

## 2. DiagramColorPreset の owned 化と effective preset helper

### 目的

`RenderInput` 由来の snapshot から `DiagramColorPreset` を組み立てられるように、preset 内部表現を owned 対応にする。

### タスク

- [x] 2.1 `DiagramColorPreset` の `&'static str` field を `Cow<'static, str>` に変更する（既存 static 値は `Cow::Borrowed`）
- [x] 2.2 `Hash` / `Clone` / `Default` 派生と既存呼び出しが通ることを確認する
- [x] 2.3 `DiagramColorPreset::for_render_input(input: &RenderInput) -> DiagramColorPreset` を追加する
- [x] 2.4 `for_render_input()` は `input.context.theme.is_some()` のとき snapshot から preset を構築し、それ以外は `DiagramColorPreset::current()` を fallback として返す
- [x] 2.5 `for_render_input()` の単体テスト（snapshot 経由 / fallback の両 path）を追加する

### Definition of Done

- [x] `DiagramColorPreset` が owned 値も保持できる
- [x] `for_render_input()` が snapshot 優先 / global fallback の順で動作する

---

## 3. Mermaid / Draw.io renderer の preset 引数化

### 目的

renderer が `RenderInput` から組み立てた preset を描画層に渡し、`DiagramColorPreset::current()` の直接参照を撤去する。

### タスク

- [x] 3.1 `MermaidRenderOps::render_mermaid_with_runtime_path()` の signature を `(block, mermaid_js, preset: &DiagramColorPreset)` に変更する
- [x] 3.2 `crates/katana-canvas-forge/src/markdown/mermaid_renderer/render.rs` から `DiagramColorPreset::current()` 直接参照を撤去し、引数の preset を使う
- [x] 3.3 `cache_file_path()` も引数の preset で hash するよう更新する
- [x] 3.4 `DrawioRendererOps::render_drawio_with_runtime_path()` も同様に preset 引数を受け取るよう変更する
- [x] 3.5 Draw.io 側の描画 / cache path も引数 preset を使うよう更新する
- [x] 3.6 `MermaidRenderer::render_block()` / `DrawioRenderer::render_block()` で `DiagramColorPreset::for_render_input(input)` を呼び、描画層へ渡す
- [x] 3.7 既存呼び出し元（scripts や internal helper）を新 signature に追従する

### Definition of Done

- [x] renderer / 描画層から `DiagramColorPreset::current()` 直接参照が無くなる（fallback の `for_render_input()` 内部のみ）
- [x] `just check` と既存単体テストが通る

---

## 4. CacheFingerprintOps を effective theme 化

### 目的

cache fingerprint を `RenderInput` 由来の実効 theme で計算し、global state ではなく consumer 指定の theme 差分で fingerprint が変わるようにする。

### タスク

- [x] 4.1 `CacheFingerprintOps::hash_current_theme()` を `hash_effective_theme(hasher, input)` に置き換える
- [x] 4.2 `hash_effective_theme()` は `DiagramColorPreset::for_render_input(input)` の各 field を hash する
- [x] 4.3 既存 test `render_fingerprint_changes_with_current_theme` を fallback path 用に維持する
- [x] 4.4 新 test `render_fingerprint_changes_with_render_input_theme` を追加し、`theme` snapshot 差分で fingerprint が変わることを確認する

### Definition of Done

- [x] fingerprint が `RenderInput.context.theme` の差分で確実に変わる
- [x] global state fallback path も引き続き fingerprint を変える

---

## 5. 回帰テスト

### 目的

Issue #4 の症状（global が dark のとき light input が無視される）が再発しないことを保証する。内部の描画アルゴリズムは変えず、外部指定値で既定 preset を上書きできることを確認する。

### タスク

- [x] 5.1 `for_render_input_prefers_render_input_theme_over_global_state` テストを追加する
  - `DARK_MODE` を true に設定
  - `RenderInput.context.theme = Some(light)` から組み立てた preset が light 値であることを確認
- [x] 5.2 Mermaid runtime request の `theme` / `background` / `fill` / `text` / `stroke` / `arrow` が preset 由来であることを確認する
- [x] 5.3 Draw.io runtime request の `dark_mode` / `background` が preset 由来であることを確認する
- [x] 5.4 `render_input_theme_ignores_global_state_for_fingerprint` テストで、snapshot 指定時に global state が fingerprint を上書きしないことを確認する

### Definition of Done

- [x] Issue #4 の再発を防ぐ test が preset / request / fingerprint の各層に存在する
- [x] score 評価や全fixture画像比較に依存していない

---

## 6. Release / CI gate と OpenSpec archive

### 目的

release PR 作成時に必要な検査を漏らさず、merge 後の `master` で同じ Rust full check を二重実行しないようにする。

### タスク

- [x] 6.1 `lefthook.yml` に `release/v*` 限定の `pre-pr` archive gate を追加する
- [x] 6.2 `scripts/release/` に対象 version 以前の active OpenSpec change を検出する script を追加する
- [x] 6.3 `just release-check` / `release-preflight` で同じ archive gate を再利用する
- [x] 6.4 CI の `pull_request` は差分種別に関係なく Rust full check を実行する
- [x] 6.5 CI の `push` は merge commit では Rust full check を省き、直接 push では動作影響差分だけ Rust full check を実行する
- [x] 6.6 `openspec/changes/v0-1-2-mermaid-zenuml-fixture-support` を archive へ移動する

### Definition of Done

- [x] `lefthook run pre-pr` が `release/v*` branch で archive 漏れを検出する
- [x] release PR merge 後の `master` push で Rust full check が再実行されない
- [x] `master` 直接 push では文書・script・workflow 差分だけなら Rust full check が省かれる

## 7. Final Verification

- [x] 7.1 `just check` を実行する
- [x] 7.2 theme / request / fingerprint の対象テストが `just check` 内で通ることを確認する
- [x] 7.3 `git diff --check` を実行する
- [x] 7.4 PR 作成前に `lefthook run pre-pr` を実行する
- [x] 7.5 `npx -y @fission-ai/openspec validate "v0-1-3-render-input-theme-application" --strict` を実行する
- [x] 7.6 KatanA adapter は本 change の非目標とし、kcf 側では外部指定を受け取る DTO / request 経路だけを実装する
