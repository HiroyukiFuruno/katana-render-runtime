# repository 標準化メモ

## 決定事項

- 対象 repository の正本名は `katana-render-runtime`。
- 作業 branch は公開配布に関わる場合 `release/vX.Y.Z` を使う。
- `katana-render-runtime` は KDR の版番号（version）を引き継ぎ、`v0.3.0` から公開する。
- 公開対象は GitHub リリース（GitHub Release）、`katana-render-runtime` crate、`katana-diagram-renderer` wrapper crate、`katana-render-runtime-cli` crate。
- KML 標準の検査名に合わせ、通常検査は `Test and Build (...)`、release 前検査は `preflight` とする。

## 対応項目

- [x] `v0.3.0` rename release 前提を版番号（version）と release 手順へ反映する。
- [x] CI を `test-and-build.yml` に統一する。
- [x] release 前検査（release preflight）を KRR の公開面へ合わせる。
- [x] repo-local skill の対象 repository 表記を KRR へ合わせる。
- [x] OpenSpec 実行入口の `scripts/openspec` を追加する。
- [x] ブランチ保護（branch protection）の必須検査を反映する。
- [x] `just` による検証を通す。
