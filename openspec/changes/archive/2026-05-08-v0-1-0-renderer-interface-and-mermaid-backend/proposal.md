## Why

KatanA から Mermaid / Draw.io / export / 採点評価の責務をKDRへ移管する。KDR v0.1.0 は新規の簡略実装ではなく、KatanA 既存機能の忠実移植（transfer）を目的とする。

責務再整理後、KDRの最終責務は外部描画である。v0.1.0で移管したHTML/PDF/PNG/JPEG exportは、KDVに同等機能が入るまで維持し、KDV実装後に移譲・削除する。

PR #1 は v0.1.0 の土台にしない。PR #1 は close 済みで、レビュー履歴だけを失敗パターンと論点整理の参照資料として扱う。

## What Changes

- KatanA 側でできていた Mermaid / Draw.io / export / score の呼び出し能力を落とさない kdr 公開 API を提供する
- KatanA 側の既存 interface と中立 DTO は移植時の照合元にし、kdr として自然な API へ整理してよい
- API 整理で field や format を削る場合は、KatanA 側の既存機能が落ちないことを KDR 側の検証で確認する
- CLI などの最表層で未指定に仕様上の意味がある入力だけ `Option` を許し、renderer 内部へ入る前に error first で非 null 値へ解決する
- `just coverage` は行カバレッジ（line coverage）100%、未到達行（uncovered line）0 を v0.1.0 の必須ゲートにする
- KatanA `crates/katana-core/src/markdown/mermaid_renderer/` の実装を kdr 側へ移管する
- KatanA `crates/katana-core/src/markdown/drawio_renderer/` の実装と resource 一式を kdr 側へ移管する
- KatanA `crates/katana-core/src/markdown/export/` の HTML / PDF / PNG / JPEG export 実装を kdr 側へ移管する
- KatanA `scripts/mermaid/`、`scripts/drawio/`、`assets/fixtures/mermaid_parts/`、`assets/fixtures/drawio/`、関連 test の検証観点を kdr 側へ移管する
- Mermaid / Draw.io の公式 reference SVG / PNG を git 管理し、ImageMagick 採点評価を kdr CLI と CI に組み込む
- CI/CD は公式 reference を再取得・再生成せず、git 管理済み reference と kdr 出力だけを比較する
- ローカルは全量評価（full evaluation）、継続的統合 / 継続的配信（CI/CD）は代表ケース評価（representative evaluation）に分ける
- `basic` fixture は疎通確認（smoke check）専用とし、vendor 互換性の保証として扱わない
- `kdr mermaid ...` / `kdr drawio ...` / `kdr export ...` CLI を library の薄い利用者として本実装にする
- `egui` / KatanA UI state が `cargo tree` に含まれないことを確認する
- `v0.1.0` として release tag を切る

## Non-Goals

- PR #1 の簡略 Mermaid 実装、HTML のみ export、SVG 文字列比較 score を土台にしない
- KatanA 既存実装より機能を削った MVP を v0.1.0 完了扱いにしない
- KatanA 側の既存機能が落ちる kdr 独自 contract を新設しない
- export CSS debug は要件変更により v0.1.x の release 番号から外し、旧 export/debug 論点はKDV移譲記録で扱う
- Mermaid.js / Draw.io.js の取り込み version 固定、最新版確認、取り込み just recipe は v0.1.1 で扱う
- Mermaid ZenUML / unsupported fixture handling は v0.1.2 で扱う
- Draw.io / Mermaid の score 改善は v0.1.4 の score 改善フェーズで扱う
- CSV / PDF / Word / Excel / PPTX viewer rendering は v0.1.0 transfer には含めない
- 新規のMarkdown viewer/export責務をKDRへ追加しない。KDV実装後は既存exportをKDVへ移譲する

## Capabilities

### New Capabilities

- `renderer-runtime-interface`: Mermaid / Draw.io rendering contract、runtime asset loading、reference scoring
- `exporter-interface`: HTML / PDF / PNG / JPEG export の `Exporter` 実装
- `reference-scoring`: Mermaid / Draw.io の公式 reference 生成、画像正規化、ImageMagick 採点評価

## Impact

- `crates/katana-diagram-renderer/src/renderer/` — trait + DTO 確定
- `crates/katana-diagram-renderer/src/exporter/` — HTML / PDF / PNG / JPEG export 実装
- `crates/katana-diagram-renderer/src/mermaid/` — KatanA から移管した Mermaid backend
- `crates/katana-diagram-renderer/src/drawio/` — KatanA から移管した Draw.io backend
- `crates/katana-diagram-renderer-cli/src/` — CLI 実装
- `vendor/mermaid/` / `vendor/drawio/` — v0.1.0 transfer で必要な runtime asset 配置
- `tests/fixtures/mermaid/` / `tests/fixtures/drawio/` — KatanA から移管した fixtures
- `tests/fixtures/mermaid/` / `tests/fixtures/drawio/` — git 管理済み公式 reference SVG / PNG と入力 fixture
- `tests/fixtures/mermaid/representative/` / `tests/fixtures/drawio/representative/` — CI/CD 用の代表ケース fixture
- `tests/reference/` — reference image scoring tests
- `docs/` — 移管手順・採点方針
