---
name: self-review
description: katana-diagram-renderer の差分をコミットや PR 前に自己レビューする。設計、テスト、品質ゲート、公開 API、描画ランタイムと CLI の境界を確認するときに使う。
---

# Self Review

現在の差分を対象に、コミットや PR に進める状態かを確認します。
既存の無関係な問題は巻き込まず、見つけた場合は OpenSpec や tasks.md に記録します。

## 1. 範囲確認

最初に確認します。

```bash
git status --short
git diff --stat
```

- 自分の変更と他者の変更を混ぜない。
- 未追跡ファイルを黙って含めない。
- 変更範囲が OpenSpec task と一致しているか確認する。

## 2. 設計確認

- library と CLI の責務が混ざっていない。
- 公開 API は最小で、内部実装を漏らしていない。
- 描画器（renderer）と CLI の境界が明確である。
- 外部コマンド（external command）、vendor bundle、チェックサム（checksum）、版固定（version pinning）の失敗が型で表現されている。
- 仕様化されていない fallback を追加していない。
- UI state、editor/preview、WebView、React の都合を入れていない。

## 3. Rust 品質確認

- 関数は 30 行前後に収まっている。
- ネストは深くしない。
- `unwrap`、`expect`、`panic!`、`todo!`、`unimplemented!`、`dbg!` を安易に追加していない。
- `println!` / `eprintln!` は CLI の出力責務として必要な場所にだけ置いている。
- コメントは WHY だけを日本語で残している。
- テスト都合で商用コードを曲げていない。

## 4. テスト確認

バグ修正では、修正前に失敗する再現テストがあることを確認します。

- library の unit test
- crate 境界をまたぐ integration test
- CLI の入力、終了コード、標準出力、標準エラー
- Mermaid/Draw.io/export の失敗経路
- checksum や version mismatch

固定待ちや sleep に頼ったテストを追加しません。

## 5. 品質ゲート

`/lint-and-ast-lint` を使い、必要な検査を通します。

標準の最小セット:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`just lint`、`just ast-lint`、`make lint` が追加されている場合は、そちらを優先します。

## 6. OpenSpec 確認

OpenSpec change 中なら確認します。

- 完了した task だけ `[x]` になっている。
- ユーザーフィードバックは `[/]` として追跡されている。
- 仕様変更が出た場合、artifact が更新されている。

## 報告形式

```markdown
# Self Review: <対象>

## 結論
PASS / FAIL

## 確認した差分
- <ファイル>

## 検証結果
- <コマンド>: PASS / FAIL

## 指摘
- なし / 修正が必要な内容
```

FAIL のままコミットや PR に進みません。
