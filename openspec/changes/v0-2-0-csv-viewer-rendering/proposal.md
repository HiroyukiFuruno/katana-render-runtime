## Why

v0.1.x で Mermaid / Draw.io / export / score の移植と export debug による表示確認の入口を作った後、最初の viewer rendering 拡張として CSV を扱う。

CSV は単なる text preview ではなく、列、行、区切り文字、引用符、文字コード、欠損値、巨大ファイルの扱いで見え方が崩れやすい。kcf は KatanA UI に依存しない形で CSV を見やすい viewer artifact に変換し、KatanA 側はその artifact を表示するだけにする。

## What Changes

- CSV 入力を parse し、viewer に渡せる table artifact を生成する
- 区切り文字、引用符、改行、文字コード、header 有無を扱う
- 大きな CSV を全量 DOM 化しないため、行範囲と列範囲を指定できる rendering を提供する
- viewer artifact は HTML fragment と構造化 metadata を返す
- CLI 公開前のため、v0.2.0 では public CLI contract を固定しない
- KatanA 側で利用しやすい adapter 境界を維持する

## Non-Goals

- CSV 編集機能を作らない
- CSV から Excel / PDF / HTML export を作らない
- 自動型推論で値の意味を変更しない
- KatanA UI state、preview state、workspace state を kcf に持ち込まない
- 公開 CLI surface の固定は v0.5.0 に送る

## Capabilities

### New Capabilities

- `csv-viewer-renderer`: CSV を viewer 表示向け table artifact へ変換する
- `csv-viewer-windowing`: 巨大 CSV を行列範囲で分割 render できる
- `csv-viewer-diagnostics`: parse error と encoding / delimiter 判定を UI 非依存の診断として返す

## Impact

- `crates/katana-canvas-forge/src/viewer/` — viewer rendering trait と CSV renderer
- `crates/katana-canvas-forge/src/viewer/csv/` — CSV parse、windowing、HTML table artifact 生成
- `crates/katana-canvas-forge-cli/src/` — v0.5.0 まで非公開扱いの検証用 entrypoint
- `tests/fixtures/csv/` — normal、quoted、multiline、wide、large、encoding fixture
- `tests/fixtures/csv/` — CSV viewer artifact の実表示確認 case
- `openspec/changes/v0-2-0-csv-viewer-rendering/` — CSV viewer rendering の仕様とタスク
