## Why

責務再整理により、Office viewer renderingはKDVへ移譲する。KDRではこのchangeを実装開始しない。

Word / Excel / PPTX に限定したviewerは、KMM/KDV/KatanAのviewer/export境界で扱う。KDRは外部描画、runtime asset、reference scoreへ責務を絞る。

## What Changes

- KDR側のv0.4.0としてはOffice viewer renderingを開始しない
- KDV側OpenSpecへOffice viewerの入力、artifact、diagnostics、表示確認を移す
- KDR側には外部描画APIの追加が必要な場合だけ別changeで残す

## Non-Goals

- Office 文書の編集機能は含めない
- Office 形式への書き出しは含めない
- Word / Excel / PPTX 以外の Office format は含めない
- score 向上や既知差分の修正は v0.4.x に分離し、v0.4.0 の scope に混ぜない
- KatanA 固有の workspace state、preview state、UI state は kdr に持ち込まない
- 商用 Office アプリケーションのインストールを必須条件にしない

## Capabilities

### New Capabilities

- `office-viewer-rendering`: KDVへ移譲

## Impact

- `katana-document-viewer` — Office viewer renderingの移譲先
- `openspec/changes/v0-4-0-office-viewer-rendering/` — KDR側では移譲記録として維持
