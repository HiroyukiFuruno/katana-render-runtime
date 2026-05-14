# Tasks: katana-diagram-renderer v0.2.1 remove KDV migrated export

## 1. Definition of Ready

- [ ] 1.1 KDV v0.1.0がreleaseされている
- [ ] 1.2 KDV側でHTML / PDF / PNG / JPG export APIが定義されている
- [ ] 1.3 KatanAまたはKDV側でKDR exportに依存しない確認ができている
- [ ] 1.4 KDR v0.2.0でexportが新規公開範囲に含まれていない

## 2. Library Removal

- [ ] 2.1 KDR libraryのdocument export API利用箇所を棚卸しする
- [ ] 2.2 HTML / PDF / PNG / JPG document export処理を削除する
- [ ] 2.3 Mermaid / Draw.io rendering APIを維持する
- [ ] 2.4 runtime assetとreference scoreへの影響がないことを確認する

## 3. CLI Removal

- [ ] 3.1 KDR CLIのdocument export commandを削除する
- [ ] 3.2 CLI help、docs、integration testからexport説明を削除する
- [ ] 3.3 KDVへの移譲先をREADMEまたはdocsに記載する

## 4. Quality Gate

- [ ] 4.1 `cargo fmt --all -- --check` を実行する
- [ ] 4.2 `cargo clippy --workspace --all-targets -- -D warnings` を実行する
- [ ] 4.3 `cargo test --workspace` を実行する
- [ ] 4.4 `just lint` と `just ast-lint` を実行する
- [ ] 4.5 `npx -y @fission-ai/openspec validate v0-2-1-remove-kdv-migrated-export --strict` を実行する
