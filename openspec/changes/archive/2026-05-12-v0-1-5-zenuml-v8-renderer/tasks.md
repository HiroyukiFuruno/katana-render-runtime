# Tasks: katana-canvas-forge v0.1.5 ZenUML V8 renderer

- **標準ブランチ**: `release/v0.1.5`
- **作業ブランチ**: `feature/v0.1.5-task-x`
- **完了基準**: 全タスクが `[x]`、品質ゲート通過、PR マージ

---

## Phase 1: `@zenuml/core` vendor asset の準備

**目的**: `zenuml.js` を kcf の vendor asset として管理可能にする。

**出力先**: `crates/katana-canvas-forge/vendor/zenuml-core/<version>/`

- [x] 1.1 `@zenuml/core` の最新バージョンを確認し、`dist/zenuml.js`（CJS）を
  `crates/katana-canvas-forge/vendor/zenuml-core/<version>/zenuml.js` に配置する
- [x] 1.2 `shasum -a 256 zenuml.js > zenuml.js.sha256` を実行して checksum を生成する
- [x] 1.3 `Justfile` に `ZENUML_CORE_JS_VERSION` 変数と `verify-zenuml-core` recipe を追加する
  （`mermaid-zenuml` の `MERMAID_ZENUML_JS_VERSION` / `verify-zenuml` と同パターン）
- [x] 1.4 `just verify-zenuml-core` が通ることを確認する

**レビュー停止点**: checksum と vendor ファイルをレビューしてから Phase 2 へ進む。

---

## Phase 2: V8 ブリッジ JS の実装

**目的**: `DiagramV8Runtime` で呼べる薄いブリッジスクリプトを作る。

**出力先**: `crates/katana-canvas-forge/src/markdown/mermaid_renderer/js_runtime/render_zenuml_v8.js`

- [x] 2.1 `render_zenuml_v8.js` を作成する
  - `zenuml.js` を eval 済みの V8 コンテキストで `renderToSvg` を呼ぶ
  - グローバル変数 `__zenuml_source__` / `__zenuml_theme__` を入力、
    `__zenuml_result__` に SVG 文字列を出力する
  - エラー時は `__zenuml_error__` にメッセージを設定する
  - **注意**: `__zenuml_source__` は mermaid コードブロック全体（先頭行 `zenuml` を含む）
    が渡される。`@zenuml/core` の `renderToSvg()` に渡す前に先頭の `zenuml` 行を
    strip しなければならない。strip しない場合、`zenuml`・`title`・`Order`・`Service`
    等が余分な参加者として描画される（Spike で確認済み）
- [x] 2.2 `Node.js` でブリッジ単体動作確認スクリプトを `tmp/` に作成して実行し、
  fixture の SVG が得られることを確認する（スクリプトは commit しない）

---

## Phase 3: `ZenumlV8RenderOps` の実装

**目的**: 旧 `ZenumlBrowserRenderOps` の代替 Rust 実装。

**出力先**: `crates/katana-canvas-forge/src/markdown/mermaid_renderer/zenuml_v8_runtime.rs`

- [x] 3.1 `zenuml_v8_runtime.rs` を作成する
  - `ZenumlV8RenderOps::render(source, preset, svg_id)` を実装する
  - `DiagramV8Runtime::render` を使い、`[zenuml.js, render_zenuml_v8.js]` の順で実行する
  - `zenuml.js` は `RuntimeAsset` 経由で materialize して読み込む
  - `render_zenuml_v8.js` は `include_str!` で embed する
- [x] 3.2 `ZenumlBrowserRenderOps` の呼び出し箇所を `ZenumlV8RenderOps` に置き換える
- [x] 3.3 `cargo build` が通ることを確認する

---

## Phase 4: 旧実装の削除

- [x] 4.1 `zenuml_browser_runtime.rs` を削除する
- [x] 4.2 `zenuml_browser_runtime_tests.rs` を削除する
  （テストは Phase 5 で V8 版として再実装する）
- [x] 4.3 `render_zenuml_browser.mjs` を削除する
- [x] 4.4 `ProcessService` への `node` 依存が ZenUML 以外に残っていないか確認する
- [x] 4.5 `cargo build` が通ることを確認する

---

## Phase 5: テストと品質ゲート

- [x] 5.1 `zenuml_v8_runtime_tests.rs` を作成し、fixture の SVG が得られることを確認する
  - `<svg>` タグを含む
  - `<foreignObject>` を含まない
  - エラーなく完了する
- [x] 5.2 `just mermaid-compare tmp/zenuml-spike 0 tmp/zenuml-spike-v8` を実行して
  スコアを記録する（baseline として `tasks.md` に追記する）
  - **baseline score: 41.63** (dark theme, fixture: `28-zen-uml.md`, 2026-05-12)
  - V8 in-process renderer での初回スコア。DOM ポリフィル全スタック追加後に取得。
- [x] 5.3 `cargo test` が通ることを確認する
- [x] 5.4 `just lint` が通ることを確認する

---

## Phase 5b: `~~~zenuml` 直接コードブロック対応

**目的**: `~~~mermaid` + `zenuml` キーワード以外に、`~~~zenuml` 言語タグでも ZenUML を描画できるようにする。

- [x] 5b.1 markdown パーサのコードブロック言語タグ処理を確認し、
  `zenuml` タグを `ZenumlV8RenderOps` に直接ルーティングする実装箇所を特定する
- [x] 5b.2 `~~~zenuml` タグのルーティングを実装する
  （strip 不要のままそのまま `ZenumlV8RenderOps::render()` に渡す）
- [x] 5b.3 `tests/fixtures/` に `~~~zenuml` 直接記法のサンプル fixture を追加する
  (`tests/fixtures/mermaid/en/29-zenuml-direct.md`)
- [x] 5b.4 fixture を使った integration test を追加し、`cargo test` が通ることを確認する
  (`extracts_zenuml_direct_fence_from_committed_fixture` in `diagram_cmd_tests.rs`)

---

## Phase 6: バージョン bump と仕上げ

- [x] 6.1 workspace `Cargo.toml` の `version` を `0.1.4` → `0.1.5` に上げる
- [x] 6.2 `CHANGELOG` または release note を更新する（該当する場合）
- [x] 6.3 `just release-check` が通ることを確認する
- [x] 6.4 `openspec/changes/v0-1-5-zenuml-v8-renderer/tasks.md` の全タスクが `[x]` であることを確認する
