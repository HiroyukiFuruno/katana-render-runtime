# Mermaid 検証用ファイル（fixture）

このディレクトリは、Mermaid の Markdown 入力と公式 reference SVG / PNG を集めた検証用ファイル置き場です。

## 構成

- `en/`: 英語 fixture の全量評価（full evaluation）用。
- `ja/`: 日本語 fixture の全量評価（full evaluation）用。
- `representative/`: 継続的統合 / 継続的配信（CI/CD）で使う代表ケース評価（representative evaluation）用。

## 評価階層

- 代表ケース評価（representative evaluation）: `representative/` を使う。CI/CD の通常 gate とする。
- 全量評価（full evaluation）: `en/` と `ja/` を使う。ローカルの release validation、runtime 更新、score 改善で実行する。

公式 reference SVG / PNG は git 管理する。CI/CD では `reference-update` を実行せず、git 管理済み reference と kcf 出力だけを比較する。
