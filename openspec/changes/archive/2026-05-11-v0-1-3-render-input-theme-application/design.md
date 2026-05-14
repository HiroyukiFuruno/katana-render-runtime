## Context

KatanA は `DiagramThemeSnapshot` を作って kdr の `RenderInput` に渡している。

- `crates/katana-core/src/markdown/diagram_backend/katana_backend.rs`
  - `kdr_input()` が `RenderPolicy { background, cache_profile, .. }` と `RenderContext { theme_fingerprint, document_id }` を設定
- `crates/katana-ui/src/preview_pane/renderer_dispatch.rs`
  - `DiagramColorPreset::is_dark_mode()` に基づいて light / dark 用 `DiagramThemeSnapshot` を構築

しかし kdr 側の renderer は `RenderInput.source` だけを使い、theme 関連の field を読んでいない。

- `crates/katana-diagram-renderer/src/renderer/backends.rs:34-40`
  - `MermaidRenderer::render_block()` は `DiagramBlock { kind, source }` を作って `MermaidRenderOps::render_mermaid_with_runtime_path(&block, &runtime_path)` を呼ぶだけ
- `crates/katana-diagram-renderer/src/renderer/backends.rs:67-74` も同様
- `crates/katana-diagram-renderer/src/markdown/mermaid_renderer/render.rs:33`
  - `let preset = DiagramColorPreset::current();` で process global を読む
- `crates/katana-diagram-renderer/src/markdown/color_preset/types.rs:24`
  - `DARK_MODE: AtomicBool = AtomicBool::new(true)` で初期値が dark
- `crates/katana-diagram-renderer/src/renderer/fingerprint.rs:24`
  - `CacheFingerprintOps::hash_current_theme()` も `DiagramColorPreset::current()` を読む

そのため、consumer が light を渡しても global が dark なら dark で描画され、cache fingerprint も `RenderInput` の theme 差分では変化しない。

## Goals

- `RenderInput` の theme 情報を、Mermaid / Draw.io の runtime request に必ず使う
- cache fingerprint を `RenderInput` 由来の実効 theme で計算する
- `DiagramColorPreset::current()` の global state が dark のままでも、`RenderInput` で light が指定されれば light の preset / request を使う
- 既存 consumer (KatanA) との後方互換は、`RenderContext` に新 field を任意（Option）で追加する形で保つ

## Non-Goals

- KatanA `DiagramThemeSnapshot` の構造を kdr 側仕様に合わせて変える（KatanA v0.22.13 で扱う）
- `DiagramColorPreset` の色 palette や描画アルゴリズムを変更する
- Draw.io / Mermaid の score 評価や画像類似度を再評価する
- `DARK_MODE` の global state を撤去する

## Implementation Plan

### 1. 公開 DTO に typed theme snapshot を追加

`RenderContext` に `theme: Option<RenderThemeSnapshot>` を追加する。

```rust
// crates/katana-diagram-renderer/src/renderer/api.rs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderContext {
    pub theme_fingerprint: Option<String>,
    pub document_id: Option<String>,
    pub theme: Option<RenderThemeSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RenderThemeSnapshot {
    pub mode: RenderThemeMode, // Light | Dark
    pub background: String,
    pub text: String,
    pub fill: String,
    pub stroke: String,
    pub arrow: String,
    pub drawio_label_color: String,
    pub mermaid_theme: String,
    // ...必要に応じて DiagramColorPreset と対応する field
}
```

判断:

- `&'static str` の `DiagramColorPreset` を直接公開しない。consumer 側から `String` で渡せる owned 型にする
- font candidates / editor_font_size など描画影響の薄い field は最初は含めない。必要になったら追加する
- `theme_fingerprint` は既存の identity hint として残し、`theme` 自体の差分も fingerprint に必ず反映する

### 2. RenderInput から実効 preset を組み立てる

`DiagramColorPreset::for_render_input(input: &RenderInput) -> DiagramColorPreset` を追加する。

ロジック:

- `input.context.theme.is_some()` → `RenderThemeSnapshot` を `DiagramColorPreset` に変換
- それ以外 → `DiagramColorPreset::current()` を fallback として返す
- `DiagramColorPreset` が `&'static str` field を持つため、snapshot 由来の値は `Box::leak` ではなく、新しい owned 型 (`DiagramColorPresetOwned` 等) を導入するか、内部表現を `Cow<'static, str>` 化する

最小変更案: `DiagramColorPreset` の field を `Cow<'static, str>` に変更し、static 値は `Cow::Borrowed`、snapshot 由来は `Cow::Owned` で構築する。`Hash` / `Clone` / `Default` 派生は維持できる。

### 3. Renderer signature 変更

`MermaidRenderOps::render_mermaid_with_runtime_path()` と `DrawioRendererOps::render_drawio_with_runtime_path()` の signature を、preset を引数で受け取る形へ変更する。

```rust
pub fn render_mermaid_with_runtime_path(
    block: &DiagramBlock,
    mermaid_js: &Path,
    preset: &DiagramColorPreset,
) -> DiagramResult;
```

`render.rs:33` の `DiagramColorPreset::current()` 直接参照は撤去し、引数で受け取った preset を使う。`cache_file_path()` も同じ preset で hash する。

`backends.rs` 側で `DiagramColorPreset::for_render_input(input)` を呼び、これを描画層へ渡す。

### 4. CacheFingerprintOps の刷新

`hash_current_theme(hasher)` を `hash_effective_theme(hasher, input)` に置き換える。

- `DiagramColorPreset::for_render_input(input)` を呼んで preset を取得
- preset の各 field と `is_dark_mode` 相当の bool を hash
- `theme_fingerprint` は既存通り hash

`render()` の signature は変更しない（`input` は既に受け取っている）。

### 5. Fallback 動作の明確化

`RenderContext.theme` が `None` の場合は、`DiagramColorPreset::current()` / `DARK_MODE` の global state を使う。これは:

- 旧 consumer / 内部 test path との後方互換
- `scripts/mermaid` / `scripts/drawio` の compare 系で global を切り替える既存運用との互換

`RenderContext.theme` が `Some` の場合は、global を一切参照しない。これは描画アルゴリズムの変更ではなく、外部指定値で既定 preset を上書きする優先順位の変更として扱う。両者の優先順位を doc comment で明示する。

### 6. 回帰テスト

`crates/katana-diagram-renderer/src/renderer/backends.rs` または隣接 test module に追加:

- `light_input_renders_with_light_preset_even_when_global_is_dark`
  - `DARK_MODE` を `true` にセット
  - `RenderInput.context.theme = Some(light snapshot)` で render
  - 描画層へ渡された preset / runtime request が light 値であることを確認
- `runtime_request_uses_external_theme_values`
  - Mermaid request の `theme` / `background` / `fill` / `text` / `stroke` / `arrow` が preset 由来であることを確認
  - Draw.io request の `dark_mode` / `background` が preset 由来であることを確認
- `cache_fingerprint_changes_with_render_input_theme`
  - `CacheFingerprintOps::render` の入力で `theme` だけが異なる場合に fingerprint が異なる
  - 既存 `render_fingerprint_changes_with_current_theme` は global fallback path として維持

`fingerprint.rs` の既存 test は、新 path（`RenderInput.context.theme` 経由）に対する case を追加する形で拡張する。

## Verification

- `RenderInput.context.theme` が runtime request / preset に反映されたことを示す単体テストが追加されている
- `DARK_MODE` を true に固定しても、light の `RenderInput` が light の preset / request を使う
- `CacheFingerprintOps::render` が `RenderInput.context.theme` 差分で fingerprint を変える
- `DiagramColorPreset::current()` の直接参照が renderer / fingerprint から無くなり、fallback path にだけ残る
- `just check`
- theme 経路だけの変更なので、score 評価や全fixture画像比較は検証対象にしない
- `npx -y @fission-ai/openspec validate "v0-1-3-render-input-theme-application" --strict`

## Release Gate / CI Flow

取り込み依頼（Pull Request）では branch protection の required check を正とし、対象差分が文書だけでも full check を必ず走らせる。これにより PR 上では全 check が通らない限り merge できない。

`release/v...` から `master` へ merge された後の push event では、CI の Rust full check を再実行しない。merge 前の PR required check で同じ差分は検査済みなので、merge 後は Release workflow だけを実行する。

`master` への直接 push は、動作影響がある差分だけ Rust full check を実行する。`*.md`、`scripts/`、`.github/`、`openspec/`、`.agents/`、`docs/`、`assets/` は Rust workspace の挙動を変えない差分として扱い、Rust full check を省く。

OpenSpec archive 漏れは `lefthook` の `pre-pr` で検出する。対象は `release/v*` branch のみとし、対象 version 以前の active change が `openspec/changes/` に残っている場合は PR 作成前に失敗させる。同じ検査を `just release-check` からも呼び、GitHub Actions の `release-preflight` とローカルの判定を揃える。
