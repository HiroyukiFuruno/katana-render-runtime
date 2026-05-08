## Context

KCFは図形描画のlibraryであり、document viewerやexport pipelineのownerではない。KME / KDV / KatanAの分離後は、KCFが担うべき範囲は Mermaid / Draw.io の外部描画、runtime asset、reference scoreである。

旧 `release/v0.1.3` は、利用側GUI品質ゲートを作るために export と viewer 的な機能まで含めた。しかし、その後の設計でKDVがviewer/exportを担うことになったため、KCFに同じ責務を残すと境界が崩れる。

## Handoff Policy

旧 `release/v0.1.3` からKDVへ渡す論点:

- HTML / PDF / PNG / JPG export
- README相対パス解決
- file path付き入力、またはfile pathのみ入力
- macOS debug open
- viewer/exportの同一pipeline確認
- 利用側GUI品質ゲートの考え方

KCFに残す論点:

- Mermaid rendering
- Draw.io rendering
- runtime asset pinning
- reference score / full compare
- KDVが呼び出す外部描画API

## KDV Alignment

KDVは `katana-document-viewer` としてv0.1.0から開始する。既存の `katana-document-preview` repository は `katana-document-viewer` へrename済みである。

KDV v0.1.0はMarkdown viewerとviewer/export共通pipelineを中心にし、HTML / PDF / PNG / JPG exportの責務を持つ。CSV / PDF / Office viewerはKDV側の後続multi-format viewerで扱う。

## Branch Handling

旧 `release/v0.1.3` branchは、KCF masterへmergeしない。必要な内容はKDV側OpenSpecへ移譲し、KCF側ではこのchangeを移譲記録として残す。

## Verification

- KCF active OpenSpecにCSV / PDF / Office viewer implementationが残っていない
- KCF v0.2.0 CLI publicationがviewer/export拡張を公開範囲に含めない
- KDV v0.1.0 OpenSpecにviewer/export責務が存在する
- KCF v0.2.1にexport削除の後続changeが存在する
