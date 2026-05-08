## Context

KCFには既存のHTML / PDF / PNG / JPG exportがある。これはKatanAから切り出した経緯に由来するが、今後のownerはKDVである。

KDV側でviewer表示とexportを同じpipelineに寄せるため、KCFがdocument exportを持ち続けるとpreviewとexportの差分が再発する。KCFはdiagram renderingを提供し、KDVがdocument viewer/exportを組み立てる。

## Start Condition

v0.2.1は、KDV側で次が完了してから開始する。

- `katana-document-viewer` v0.1.0 がreleaseされている
- HTML / PDF / PNG / JPG exportの移譲先APIがKDV側で定義されている
- KatanAまたはKDV側でKCF exportに依存しない動作確認ができる

## Removal Boundary

削除するもの:

- KCF libraryのdocument export API
- KCF CLIのdocument export command
- KCF docsのdocument export説明
- export専用のCSS解釈、README相対パス解決、macOS debug open

残すもの:

- Mermaid rendering
- Draw.io rendering
- rendering結果のartifact生成
- runtime asset management
- reference score / compare

## Compatibility

KCF v0.2.0ではexportを新規公開範囲に含めない。v0.2.1の削除は、KDV移譲完了後の責務整理として扱う。

KatanAやKDVが必要とする図形描画APIは削除しない。document全体のHTML / PDF / PNG / JPG exportだけをKDVへ移す。

## Verification

- KCF CLI helpにdocument export commandが残っていない
- KCF public docsにKCF document export手順が残っていない
- KDV側のexport pipelineからKCFのMermaid / Draw.io rendering APIを呼べる
- KCFのrender / score / reference更新テストが維持されている
