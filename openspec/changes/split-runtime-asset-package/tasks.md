## 1. 配布検証の前提

- [x] 1.1 公開版と候補packageの実サイズ・資産SHAを比較して超過原因を特定する
- [x] 1.2 最小workspace fixtureで未公開資産依存のpackage検証ビルドとregistry lockfileを実証する

## 2. 資産の配布境界

- [x] 2.1 同一versionのassets crateへ既存圧縮Draw.io bytesを移し、runtime依存を接続する
- [x] 2.2 資産更新・同期検査を単一保存先へ接続し、byte一致・展開後checksumの回帰を確認する

## 3. release契約

- [ ] 3.1 全公開packageの内容・容量・依存version・未公開状態検査を更新する
- [x] 3.2 assets→runtime→CLIの公開順序と部分公開retryを回帰検証する
- [ ] 3.3 実package集合の検証ビルドと各10MiB上限、オフライン描画回帰、全品質ゲートを通す

## 4. 統合と公開

- [ ] 4.1 最新HEAD自己レビューとDraftレビュー、指摘個別reply/resolve、required CIを通す
- [ ] 4.2 保護merge後に依存順公開と公開registry consumerのKRR内受入を確認する
- [ ] 4.3 Issue #89/#95を完了条件と照合して閉じ、不要ローカルbranchを安全に整理する

## 5. ユーザーフィードバック

- [/] 5.1 効果が薄い性能try&errorを追加しない。今回の対象は測定で特定した配布容量超過であり、JS・描画・CPU処理を変更しない
