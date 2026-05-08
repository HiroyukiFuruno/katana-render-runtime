## Why

責務再整理により、PDF viewer renderingはKDVへ移譲する。KCFではこのchangeを実装開始しない。

PDF はpage、rotation、crop box、透明背景、font、画像、巨大documentの扱いで失敗しやすい。viewer/exportをKDVへ寄せる方針により、この領域は `katana-document-viewer` 側で扱う。

## What Changes

- KCF側のv0.3.0としてはPDF viewer renderingを開始しない
- KDV側OpenSpecへPDF viewerのpage artifact、metadata、diagnostics、windowingを移す
- KCF側には外部描画referenceやscoreの保守だけを残す

## Non-Goals

- PDF 編集、annotation 追加、署名、フォーム入力を作らない
- OCR や text extraction を v0.3.0 に含めない
- PDF export の実装を置き換えない
- KatanA UI state、preview state、workspace state を kcf に持ち込まない
- 公開 CLI surface の固定は v0.5.0 に送る

## Capabilities

### New Capabilities

- `pdf-viewer-renderer`: KDVへ移譲
- `pdf-page-windowing`: KDVへ移譲
- `pdf-render-diagnostics`: KDVへ移譲

## Impact

- `katana-document-viewer` — PDF viewer renderingの移譲先
- `openspec/changes/v0-3-0-pdf-viewer-rendering/` — KCF側では移譲記録として維持
