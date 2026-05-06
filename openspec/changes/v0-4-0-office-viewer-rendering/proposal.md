## Why

v0.2.0 で CSV viewer rendering、v0.3.0 で PDF viewer rendering を追加した後、KatanA 側で利用する canvas backend は Office 文書の表示にも対応する必要がある。

v0.4.0 では Word / Excel / PPTX に限定して、編集や Office 書き出しではなく、表示確認 case と同じ考え方で表示結果を検証できる rendering surface を追加する。

## What Changes

- Word / Excel / PPTX を読み込み、表示用 artifact へ変換する viewer rendering を追加する
- Office 入力の検証、失敗理由、対応 format を公開 API で表現する
- Office rendering は KatanA UI state に依存しない generic repository の責務として定義する
- KatanA 側では生成済み artifact を canvas 表示へ渡す consumer として利用できるようにする
- Word は page、Excel は sheet / range、PPTX は slide を表示単位として扱う
- 表示確認 case、reference snapshot、差分比較、lint、AST lint、release gate を品質確認に含める

## Non-Goals

- Office 文書の編集機能は含めない
- Office 形式への書き出しは含めない
- Word / Excel / PPTX 以外の Office format は含めない
- score 向上や既知差分の修正は v0.4.x に分離し、v0.4.0 の scope に混ぜない
- KatanA 固有の workspace state、preview state、UI state は kcf に持ち込まない
- 商用 Office アプリケーションのインストールを必須条件にしない

## Capabilities

### New Capabilities

- `office-viewer-rendering`: Word / Excel / PPTX を表示用 artifact に変換し、generic library として利用できる

## Impact

- `crates/katana-canvas-forge` — Office viewer rendering の公開 API、入力検証、変換処理
- `crates/katana-canvas-forge-cli` — 実装確認用の内部 CLI 経由呼び出し
- `tests/fixtures/office/` — Office rendering の表示確認 case と reference snapshot
- `tests/fixtures/office/` — Word / Excel / PPTX の最小 case と代表 case
- `Justfile` — Office reference 更新、表示確認、品質 gate recipe
- `openspec/changes/v0-4-0-office-viewer-rendering/` — 本 change の仕様とタスク
