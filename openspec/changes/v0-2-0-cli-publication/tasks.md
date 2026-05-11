## 1. Public CLI Contract

### Definition of Ready

- [ ] v0.1.0 から v0.1.5 までの Mermaid / Draw.io renderer、score、reference 更新、KDV移譲記録を確認している
- [ ] CLI が library の薄い利用者である方針が固定されている
- [ ] KatanA 固有 state を CLI argument にしない方針が固定されている

### 目的

CLI の公開 command、argument、output、exit code を固定する。

### 書き込み範囲

- `crates/katana-canvas-forge-cli`
- `crates/katana-canvas-forge`
- `openspec/changes/v0-2-0-cli-publication`

### タスク

- [ ] 1.1 binary 名と public command 一覧を固定する
- [ ] 1.2 `--help`、`--version`、render、score、reference 更新の output contract を固定する
- [ ] 1.3 exit code と stderr の方針を固定する
- [ ] 1.4 machine readable output が必要な command に schema test を追加する
- [ ] 1.5 CLI が library の公開 API を経由して処理することを確認する
- [ ] 1.6 既存 export command は互換維持対象として棚卸しし、新規拡張しないことを明記する

### Definition of Done

- [ ] 公開 command が docs と test に一致している
- [ ] KatanA 固有 state が CLI contract に含まれていない
- [ ] output contract の破壊的変更を test で検出できる
- [ ] CSV / PDF / Office viewer rendering がKCF CLI公開範囲へ戻っていない

---

## 2. Package And Install Documentation

### 目的

crates.io 公開と install 手順を整える。

### 書き込み範囲

- `Cargo.toml`
- `crates/katana-canvas-forge-cli/Cargo.toml`
- `README.md`
- `docs`

### タスク

- [ ] 2.1 package name、binary name、license、repository、description、readme、keywords、categories を確認する
- [ ] 2.2 `cargo install` の install 手順を docs に追加する
- [ ] 2.3 command usage、input / output、exit code、error message の例を docs に追加する
- [ ] 2.4 package include / exclude を確認し、fixture、snapshot、vendor cache の混入を防ぐ
- [ ] 2.5 docs の内容が CLI help と矛盾しないことを test または release gate で確認する

### Definition of Done

- [ ] 公開 package metadata が crates.io publish 前提で揃っている
- [ ] install 手順が copy 可能である
- [ ] package に不要な大型 artifact が含まれていない

---

## 3. CI And Release Dry Run

### 目的

公開前に CI と dry run で失敗を検出する。

### 書き込み範囲

- `.github/workflows`
- `Justfile`
- `scripts`

### タスク

- [ ] 3.1 CLI integration test を CI に含める
- [ ] 3.2 `cargo package --list` を release gate に含める
- [ ] 3.3 `cargo publish --dry-run` を release gate に含める
- [ ] 3.4 CLI smoke test を release dry run に含める
- [ ] 3.5 KatanA consumer compatibility check を release gate に含める
- [ ] 3.6 KDVへ移譲した CSV / PDF / Office viewer rendering を release gate から除外する

### Definition of Done

- [ ] CI で CLI の公開 contract が検証される
- [ ] crates publish dry run が成功している
- [ ] release dry run が package 内容と CLI smoke を確認している

---

## 4. KatanA Consumer Compatibility

### 目的

KatanA 側で利用する前提を generic repository の境界内で検証する。

### 書き込み範囲

- `test/fixtures`
- `crates/katana-canvas-forge-cli`
- `crates/katana-canvas-forge`

### タスク

- [ ] 4.1 KatanA consumer が必要とする render、score、reference 更新の最小 fixture を定義する
- [ ] 4.2 CLI output と library result の metadata が consumer で解釈できることを確認する
- [ ] 4.3 field 削除、exit code 変更、error code 変更を breaking change として検出する
- [ ] 4.4 KatanA 固有の UI state を compatibility fixture に含めない
- [ ] 4.5 KDVへ移譲した viewer/export fixture をKCF互換性fixtureに含めない

### Definition of Done

- [ ] KatanA 利用前提が docs と release gate に明記されている
- [ ] generic repository としての API 境界が崩れていない
- [ ] compatibility check が CI または release dry run で実行される

---

## 5. Quality Gate

- [ ] 5.1 `cargo fmt --all -- --check` を実行する
- [ ] 5.2 `cargo clippy --workspace --all-targets -- -D warnings` を実行する
- [ ] 5.3 `cargo test --workspace` を実行する
- [ ] 5.4 CLI integration test を実行する
- [ ] 5.5 KDVへ移譲した viewer/export smoke がKCF側に残っていないことを確認する
- [ ] 5.6 `/lint-and-ast-lint` を実行し、静的検査（lint）と抽象構文木検査（AST lint）の結果を記録する
- [ ] 5.7 release dry run を実行する
- [ ] 5.8 `cargo publish --dry-run` を実行する
- [ ] 5.9 `/self-review` を実行する
- [ ] 5.10 `npx -y @fission-ai/openspec validate v0-2-0-cli-publication --strict` を実行する

---

## 6. User Review

> ユーザーから受けた指摘は `[/]` で閉じる。通常の開発タスク `[x]` と混ぜない。

- [ ] 6.1 実装結果、CLI contract、package dry run、release gate 結果をユーザーに提示する
- [ ] 6.2 フィードバックを本 `tasks.md` に追記し、対応済みを `[/]` にする
