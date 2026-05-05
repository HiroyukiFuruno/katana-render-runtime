## Why

v0.3.0 では PDF viewer rendering を追加する。v0.1.0 の PDF export は「Markdown などから PDF を書き出す」責務であり、v0.3.0 の PDF viewer rendering は「既存 PDF を画面表示向け artifact に変換する」責務である。

PDF は page、rotation、crop box、透明背景、font、画像、巨大 document の扱いで失敗しやすい。kcf が UI 非依存で page artifact を生成し、KatanA 側は artifact を viewer に渡すだけにする。

## What Changes

- PDF 入力を page 単位の viewer artifact に render する
- page count、page size、rotation、render scale、diagnostics を metadata として返す
- page range を指定して必要な page だけ render できる
- rendering backend の初期化と error を構造化して返す
- viewer e2e で PDF page artifact の実表示を確認する

## Non-Goals

- PDF 編集、annotation 追加、署名、フォーム入力を作らない
- OCR や text extraction を v0.3.0 に含めない
- PDF export の実装を置き換えない
- KatanA UI state、preview state、workspace state を kcf に持ち込まない
- 公開 CLI surface の固定は v0.5.0 に送る

## Capabilities

### New Capabilities

- `pdf-viewer-renderer`: PDF を page artifact へ render する
- `pdf-page-windowing`: page range と scale を指定して render できる
- `pdf-render-diagnostics`: backend 初期化、password、破損 PDF、unsupported feature を構造化 error として返す

## Impact

- `crates/katana-canvas-forge/src/viewer/pdf/` — PDF render backend、page artifact、metadata
- `tests/fixtures/pdf/` — normal、multi-page、rotated、large、invalid fixture
- `test/e2e/viewer/` — PDF page artifact の実表示確認
- `Justfile` — v0.5.0 まで非公開扱いの検証 recipe
- `openspec/changes/v0-3-0-pdf-viewer-rendering/` — PDF viewer rendering の仕様とタスク
