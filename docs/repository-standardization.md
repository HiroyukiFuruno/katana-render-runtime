# repository 標準化メモ

## 決定事項

- 対象 repository は `katana-diagram-renderer`。
- 作業 branch は `master`。
- 初回公開版番号（version）は `v0.1.0` から開始する。
- 公開対象は GitHub リリース（GitHub Release）、`katana-diagram-renderer` crate、`katana-diagram-renderer-cli` crate。
- KML 標準の検査名に合わせ、通常検査は `Test and Build (...)`、release 前検査は `preflight` とする。

## 対応項目

- [x] `v0.1.0` 開始前提を版番号（version）と release 手順へ反映する。
- [x] CI を `test-and-build.yml` に統一する。
- [x] release 前検査（release preflight）を KDR の公開面へ合わせる。
- [x] repo-local skill の対象 repository 表記を KDR へ合わせる。
- [x] OpenSpec 実行入口の `scripts/openspec` を追加する。
- [x] ブランチ保護（branch protection）の必須検査を反映する。
- [x] `just` による検証を通す。
