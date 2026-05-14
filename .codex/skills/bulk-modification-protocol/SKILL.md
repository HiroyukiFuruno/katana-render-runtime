---
name: bulk-modification-protocol
description: katana-diagram-renderer で複数ファイルの一括置換、削除、移動、生成を行う前に使う安全手順。事前確認、分割実行、diff 精査、ユーザー承認を強制する。
---

# Bulk Modification Protocol

一括変更は速さではなく、事故りやすい操作です。
複数ファイルをまとめて書き換える前に、この手順で安全性を確保します。

## 適用条件

次のどれかに当てはまる場合に使います。

- 複数ファイルへの置換や削除
- `find`、`xargs`、`sed`、`awk`、スクリプトによる自動書き換え
- 抽象構文木（AST）を使った大規模リファクタリング
- 生成物の大量追加や移動
- `vendor/` や `crates/` をまたぐ構造変更

## 1. 事前確認

```bash
git status --short
git diff --stat
```

- 既存差分の所有者を確認する。
- 他者の差分を戻さない。
- 安全な checkpoint が必要なら、ユーザーに確認してから commit または退避する。
- ユーザーの承認なしに破壊的コマンドを実行しない。

## 2. バッチを分ける

責務で小さく分けます。

- public API
- renderer
- CLI
- vendor bundle
- OpenSpec
- tests

同じバッチで無関係な責務を混ぜません。

## 3. 実行する

実行前に、対象パスと操作内容を短く報告します。
コマンドは対象を絞り、リポジトリ全体への無差別置換を避けます。

## 4. diff を読む

各バッチ後に必ず確認します。

```bash
git diff --stat
git diff
```

確認観点:

- 残すべき WHY コメントを消していないか。
- checksum、version、fixture、expected output を巻き込んでいないか。
- public API と crate 境界を壊していないか。
- テストの期待値を検証目的から逸らしていないか。

## 5. 一時停止する

一括変更の後は、検証結果と diff の要点をユーザーに報告して止まります。
承認なしに commit へ進みません。

## 禁止

- `git reset --hard`
- `git checkout .`
- `git clean -fd`
- lint 除外や allow の追加で失敗を隠すこと
- 失敗した一括変更を別の手段で迂回実行すること
