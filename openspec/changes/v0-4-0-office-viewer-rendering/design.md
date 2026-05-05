## Context

kcf は KatanA の canvas backend として使われるが、repository 自体は KatanA UI state に依存しない generic library として保つ。

v0.4.0 の責務は Office 文書を「表示できる artifact」へ変換することに限定する。KatanA 側は、その artifact を canvas に載せる consumer であり、Office 文書の解析状態や UI 操作状態を kcf に渡さない。

## Goals

- Word / Excel / PPTX の viewer rendering を同じ API surface で扱う
- 入力 format 判定、変換失敗、未対応 feature を型で表現する
- reference snapshot と viewer e2e で表示品質を確認する
- CLI、lint、AST lint、release gate から実装品質を確認できる状態にする

## Non-Goals

- Office 編集、注釈編集、セル編集、スライド編集
- Office format への export
- KatanA 固有 UI の状態管理
- v0.4.x での追加機能分割

## Design

### Rendering Boundary

Office viewer renderer は、入力ファイルと rendering option を受け取り、HTML / SVG / PNG / PDF などの表示用 artifact を返す。artifact の最終 format は実装時に決めるが、consumer が KatanA であっても API は KatanA 固有型を受け取らない。

表示単位は format ごとに分ける。

- Word: page または section を単位にする
- Excel: sheet と range を単位にする
- PPTX: slide を単位にする

Excel は v0.2.0 の CSV と同じく巨大 sheet を全量 DOM 化しない。sheet / row / column range を指定できる rendering option を持つ。

### Supported Formats

- Word: `.docx`
- Excel: `.xlsx`
- PPTX: `.pptx`

旧 Office binary format、マクロ付き format、パスワード付き文書は未対応として明示的な error にする。

## Rendering Engine Ownership

Office rendering engine は実装時に選定する。ただし、商用 Office application を必須にしてはならない。

選定時は次を固定する。

- engine name と version
- bundle / binary / external process の checksum
- macOS / Linux / CI での setup 方法
- unsupported feature の warning / error policy
- reference snapshot 更新 recipe

### Quality Policy

Office rendering は完全な編集互換ではなく表示互換を目標にする。文字、表、画像、ページ境界、シート境界、スライド境界が reference と比較可能な artifact になることを重視する。

### KatanA Consumer Compatibility

KatanA 側は、kcf の Office rendering result を canvas backend の入力として利用する。kcf は consumer compatibility を確認するため、公開 API の破壊的変更、artifact metadata、error code の互換性を release gate で確認する。

## Risks

- Office rendering engine の差で platform ごとの表示差分が出る
- Excel の広い sheet や PPTX の埋め込み media は表示負荷が高い
- Office 文書には unsupported feature が多く、error と degraded rendering の境界が曖昧になりやすい

## Mitigations

- reference snapshot を最小 case と代表 case に分ける
- unsupported feature は fallback ではなく明示的な warning または error にする
- viewer e2e と score を併用し、見た目確認と自動比較の責務を分ける
- score 向上は v0.4.x に分離し、v0.4.0 は rendering surface と評価基盤の完成を優先する
