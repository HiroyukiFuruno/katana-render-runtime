# Tasks: katana-canvas-forge v0.1.3 Mermaid ZenUML fixture support

## Branch Rule

- **標準ブランチ**: `release/v0.1.3`
- **作業ブランチ**: `feature/v0.1.3-task-x`

---

## 1. Fixture Policy

### 目的

`28-zen-uml.md` を supported にするか unsupported として明示するかを決める。

### タスク

- [ ] 1.1 Mermaid.js runtime の ZenUML support 状態を確認する
- [ ] 1.2 `en/28-zen-uml.md` と `ja/28-zen-uml.md` の扱いを固定する
- [ ] 1.3 unsupported とする場合は reason metadata を定義する

### Definition of Done

- [ ] supported / unsupported の判断が artifact に残っている

---

## 2. Compare Error Handling

### 目的

full compare が unsupported fixture や空出力で null 参照しないようにする。

### タスク

- [ ] 2.1 rasterize / compare の null 参照箇所を error first に直す
- [ ] 2.2 unsupported fixture を report に出す
- [ ] 2.3 fallback SVG / stub PNG を使っていないことをテストする

### Definition of Done

- [ ] full compare が原因付きで完了または失敗する
- [ ] null 参照で落ちない

---

## 3. ZenUML Support Or Explicit Unsupported

### 目的

ZenUML を描画可能にするか、未対応として明示的に扱う。

### タスク

- [ ] 3.1 support 可能な場合は runtime 初期化を追加する
- [ ] 3.2 support できない場合は unsupported report を実装する
- [ ] 3.3 supported の場合は score reference を更新する
- [ ] 3.4 unsupported の場合は score 対象から外した理由を report に残す

### Definition of Done

- [ ] ZenUML の扱いが曖昧でない
- [ ] supported fixture を暗黙に除外していない

---

## 4. Final Verification

- [ ] 4.1 `mermaid-compare-full` を実行する
- [ ] 4.2 `just check` を実行する
- [ ] 4.3 `git diff --check` を実行する
- [ ] 4.4 PR 作成後に `@codex review` を依頼する
- [ ] 4.5 `npx -y @fission-ai/openspec validate "v0-1-3-mermaid-zenuml-fixture-support" --strict` を実行する
