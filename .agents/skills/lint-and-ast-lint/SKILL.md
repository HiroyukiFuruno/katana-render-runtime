---
name: lint-and-ast-lint
description: katana-canvas-forge の静的検査と抽象構文木検査を厳格に実行する。renderer/export runtime の品質ゲート、clippy、format、test、禁止パターン確認、allow や exclude の不正追加防止で使う。
---

# Lint and AST Lint

このスキルは、kcf の品質ゲートを「通ればよい」ではなく「設計意図を守る検査」として扱います。
描画と書き出しの実行基盤（renderer/export runtime）では、失敗経路、version pinning、checksum、外部プロセス境界を弱めないことを重視します。

## 1. 入口を確認する

自己流コマンドを先に使いません。

1. `Justfile` があれば `just --list` または `just --summary` を確認する。
2. `Makefile` があれば `make help` を確認する。
3. `just lint` / `just ast-lint` / `make lint` / `make ast-lint` があれば、それを優先する。
4. 専用入口がない場合だけ、Cargo の標準ゲートを使う。

専用入口がない場合の標準ゲート:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 2. lint の扱い

clippy warning は失敗として扱います。
次の対応は禁止です。

- `#[allow(...)]` を理由なしに追加する。
- lint 設定から対象を外す。
- テストだけ緩くする。
- `unwrap` や `expect` を「ここでは大丈夫」として増やす。
- エラーを fallback で握りつぶす。

修正順序:

1. 既存コードの近い実装を確認する。
2. ルールに従う形へ設計を直す。
3. ルールに従えない場合は、理由と代替案をユーザーに確認する。

## 3. AST lint の扱い

抽象構文木検査（AST lint）は、文字列検索より強い構文ベースの検査です。
`just ast-lint` や専用 harness が存在する場合は必ず実行します。

専用 AST lint がまだない場合、次の検索は「調査用」です。正式な AST lint 合格とは呼びません。

```bash
rg -n "unwrap\\(|expect\\(|panic!|todo!|unimplemented!|dbg!" crates
rg -n "egui|WebView|React|TypeScript|editor|preview|UiState" crates
rg -n "serde_json::Value|Box<dyn Any>|HashMap<.*Value" crates
```

調査で問題が見つかった場合は、構文ベースの検査へ昇格するか、今回の修正で直接直します。

## 4. kcf 固有の禁止パターン

次は renderer/export runtime の品質を落とします。

- UI state に依存する型や引数
- renderer と exporter の相互依存
- CLI からしか使えない library API
- version pinning のない vendor bundle
- checksum 不一致を warning 扱いにすること
- external command の exit status、stderr、timeout を捨てること
- `anyhow::Error` だけで public API の失敗種類を潰すこと
- test fixture の期待値を実装に合わせて安易に更新すること

## 5. 報告形式

```markdown
# Lint and AST Lint

## 結論
PASS / FAIL

## 実行した入口
- <command>

## 結果
- format:
- clippy:
- test:
- AST lint:

## 対応
- 修正した内容 / 未解決事項
```

専用 AST lint が存在しない場合は、`AST lint: 未整備。調査用検索のみ実施` と明記します。
