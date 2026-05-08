# Tasks: katana-canvas-forge v0.1.6 KDV export handoff

## 1. Scope Baseline

- [x] 1.1 旧 `release/v0.1.3` の内容を確認する
- [x] 1.2 KCFに残す責務を Mermaid / Draw.io rendering、runtime asset、reference scoreへ限定する
- [x] 1.3 HTML / PDF / PNG / JPG export をKDVへ移譲する方針を記録する
- [x] 1.4 CSV / PDF / Office viewer をKDVへ移譲する方針を記録する

## 2. KDV Handoff

- [x] 2.1 `katana-document-preview` repository を `katana-document-viewer` へrenameする
- [x] 2.2 KDV v0.1.0がviewer/export pipelineを担うことを確認する
- [ ] 2.3 KDV側OpenSpecへ旧 `release/v0.1.3` の export/debug 論点を反映する
- [ ] 2.4 KDV側でREADME相対パス解決、file path付き入力、macOS debug openの扱いを決める

## 3. KCF Guardrail

- [x] 3.1 旧 `release/v0.1.3` をKCF masterへmergeしない方針を記録する
- [x] 3.2 KCF v0.2.0 CLI publicationからCSV / PDF / Office viewer renderingを外す
- [x] 3.3 KCF v0.2.1にexport削除changeを追加する
- [ ] 3.4 KDV v0.1.0実装完了後、KCF v0.2.1を開始する

## 4. Quality Gate

- [ ] 4.1 `npx -y @fission-ai/openspec validate v0-1-6-kdv-export-handoff --strict` を実行する
- [ ] 4.2 `npx -y @fission-ai/openspec validate v0-2-0-cli-publication --strict` を実行する
- [ ] 4.3 `npx -y @fission-ai/openspec validate v0-2-1-remove-kdv-migrated-export --strict` を実行する
