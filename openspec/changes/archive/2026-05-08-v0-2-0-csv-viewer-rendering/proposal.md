## Why

責務再整理により、CSV viewer renderingはKDVへ移譲する。KDRではこのchangeを実装開始しない。

CSV は単なるtext previewではなく、列、行、区切り文字、引用符、文字コード、欠損値、巨大ファイルの扱いで見え方が崩れやすい。viewer/exportをKDVへ寄せる方針により、この領域は `katana-document-viewer` 側で扱う。

## What Changes

- KDR側のv0.2.0としてはCSV viewer renderingを開始しない
- KDV側OpenSpecへCSV viewerの入力、artifact、診断、windowingを移す
- KDR側には外部描画APIの追加が必要な場合だけ別changeで残す

## Non-Goals

- CSV 編集機能を作らない
- CSV から Excel / PDF / HTML export を作らない
- 自動型推論で値の意味を変更しない
- KatanA UI state、preview state、workspace state を kdr に持ち込まない
- 公開 CLI surface の固定は v0.5.0 に送る

## Capabilities

### New Capabilities

- `csv-viewer-renderer`: KDVへ移譲
- `csv-viewer-windowing`: KDVへ移譲
- `csv-viewer-diagnostics`: KDVへ移譲

## Impact

- `katana-document-viewer` — CSV viewer renderingの移譲先
- `openspec/changes/v0-2-0-csv-viewer-rendering/` — KDR側では移譲記録として維持
