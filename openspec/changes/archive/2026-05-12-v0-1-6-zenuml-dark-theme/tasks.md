# Tasks: katana-diagram-renderer v0.1.6 ZenUML dark mode theme

- **標準ブランチ**: `release/v0.1.6`
- **作業ブランチ**: `feature/v0.1.6-task-x`
- **完了基準**: 全タスクが `[x]`、品質ゲート通過、PR マージ
- **関連 issue**: [#12](https://github.com/HiroyukiFuruno/katana-diagram-renderer/issues/12)

---

## Phase 1: JS ブリッジの theme 接続

**目的**: `render_zenuml_v8.js` が `__zenuml_dark__` を読み取り、`renderToSvg()` の
`options` に渡す。

- [x] 1.1 `render_zenuml_v8.js` を修正する
  - `__zenuml_dark__` グローバルを読み取る
  - `__zenuml_dark__ === true` のとき `{ theme: "dark" }` を `renderToSvg()` に渡す
  - `false` のときは `{}` またはオプション省略で呼ぶ

---

## Phase 2: Rust 側 preamble の修正

**目的**: `zenuml_v8_runtime.rs` が `preset.dark_mode` を preamble に注入する。

- [x] 2.1 `build_preamble(source, is_dark: bool)` にシグネチャを変更する
  - `var __zenuml_dark__ = true/false;` を出力する
- [x] 2.2 `ZenumlV8RenderOps::render()` 内の `_preset` → `preset` に変更し、
  `build_preamble(source, preset.dark_mode)` を呼ぶ
- [x] 2.3 `cargo build` が通ることを確認する

---

## Phase 3: テストと品質ゲート

- [x] 3.1 `zenuml_v8_runtime_tests.rs` にテストを追加する
  - ダークモードで `<svg>` が返ること
  - ライトモードで `<svg>` が返ること
  - `build_preamble` が `__zenuml_dark__` を正しく出力すること
- [x] 3.2 `cargo test` が通ることを確認する
- [x] 3.3 `just lint` が通ることを確認する

---

## Phase 4: バージョン bump と仕上げ

- [x] 4.1 workspace `Cargo.toml` の `version` を `0.1.5` → `0.1.6` に上げる
- [x] 4.2 `just VERSION=v0.1.6 release-check` が通ることを確認する（archive 後）
- [x] 4.3 本 tasks.md の全タスクが `[x]` であることを確認する
