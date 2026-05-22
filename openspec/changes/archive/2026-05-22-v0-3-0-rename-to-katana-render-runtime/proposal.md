## Why

`katana-diagram-renderer` は Mermaid / Draw.io / PlantUML を中心にした図形描画 crate として始まったが、KDV の export では数式も含めて「入力を安定した描画成果物へ変換する runtime」が必要になっている。

数式は図形ではないため、現行名のまま MathJax を追加すると責務名と実体がずれる。一方で、KDR には既に V8 実行基盤、runtime bundle、theme / dark-mode 伝搬、SVG 生成の実績がある。新規 repository を作り直すより、現 repository を `katana-render-runtime` へ rename して履歴を残し、crate は新名で公開するほうが移行と保守の両面で自然である。

## What Changes

- GitHub repository を `katana-render-runtime` へ rename する
- 新 crate `katana-render-runtime` を crates.io に `v0.3.0` から公開する
- 旧 crate `katana-diagram-renderer` は `v0.3.0` の互換 wrapper として残し、移行案内を出す
- 既存の Mermaid / Draw.io / PlantUML runtime を `katana-render-runtime` の機能として引き継ぐ
- MathJax による TeX to SVG runtime を追加する
- theme / dark-mode / asset / diagnostics の runtime contract を図形と数式で共通化する
- README、crate metadata、ロゴ、アイコン、badges、docs、OpenSpec の名称を `katana-render-runtime` へ更新する
- KDV は新 crate の release を待ち、公開後に dependency を切り替える

## Capabilities

### New Capabilities

- `render-runtime-repository`: repository rename、crate rename、旧 crate 互換 wrapper、README ロゴ更新、release / publish migration を管理する
- `mathjax-svg-rendering`: MathJax runtime で TeX を SVG 化し、HTML / PDF / PNG / JPG などの上位 consumer が同じ SVG を利用できるようにする

### Modified Capabilities

- `renderer-runtime-interface`: 図形専用の KDR API から、render runtime としての API へ拡張する
- `diagram-runtime-bundle-management`: runtime bundle の管理対象に MathJax を追加できる構造へ広げる
- `render-input-theme-application`: complete theme と dark-mode を、図形だけでなく数式 runtime にも渡せる contract にする

## Impact

- GitHub repository name: `katana-diagram-renderer` -> `katana-render-runtime`
- crates.io:
  - new: `katana-render-runtime = 0.3.0`
  - compatibility: `katana-diagram-renderer = 0.3.0`
- Rust crate names / package metadata
- README.md のタイトル、説明、ロゴ、アイコン、badges、migration note
- `Cargo.toml` workspace package names
- Public API docs
- Runtime bundle source / generated artifacts
- MathJax runtime bundle and checksum
- KatanA / KDV dependency migration plan
- OpenSpec project.md / specs / changes

## Definition of Done

- `katana-render-runtime v0.3.0` が crates.io に公開されている
- `katana-diagram-renderer v0.3.0` が互換 wrapper として crates.io に公開されている
- MathJax runtime が TeX input を SVG output へ変換できる
- MathJax runtime の失敗時は diagnostics 付き raw string を返す
- Mermaid / Draw.io / PlantUML の既存 runtime contract が rename 後も維持されている
- README のタイトル、説明、ロゴ、アイコン、badges、install snippet、migration note が `katana-render-runtime` 前提に更新されている
- KDV 側へ dependency switch 可能な状態として handoff されている
