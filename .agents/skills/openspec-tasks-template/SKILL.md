---
name: openspec-tasks-template
description: katana-canvas-forge の OpenSpec tasks.md を、実装者がそのまま進められる粒度へ整える。タスク分割、完了条件、品質ゲート、レビュー停止点を入れるときに使う。
---

# OpenSpec Tasks Template

`tasks.md` は実装メモではなく、別の作業者が読んでも同じ順序で実装できる契約です。
kcf では、公開 API、renderer、exporter、CLI、vendor bundle、テスト、品質ゲートを分けて書きます。

## 1. Change Directory Naming

`openspec/changes/` 配下の変更名は、version と責務が読める名前にします。

```text
v0-1-0-renderer-interface-and-mermaid-backend
v0-2-0-native-diagram-backends
```

## 2. Definition of Ready

大きいタスクグループには、最初に次を置きます。

```markdown
### Definition of Ready

- [ ] 変更対象の crate、公開 API、CLI 入口が明確である
- [ ] 前のタスクグループの実装、検証、レビューが完了している
- [ ] 外部プロセス、vendor bundle、checksum、version pinning の扱いが決まっている
```

## 3. Task Group Template

```markdown
## 1. <タスクグループ名>

### 目的

<このタスクで固定する責務を短く書く>

### 書き込み範囲

- `crates/...`
- `vendor/...`
- `openspec/...`

### タスク

- [ ] 1.1 <実装内容>
- [ ] 1.2 <テスト内容>
- [ ] 1.3 <検証内容>

### Definition of Done

- [ ] 公開 API と crate 境界が設計どおりである
- [ ] renderer/exporter の失敗経路をテストしている
- [ ] `cargo fmt --all -- --check` が通っている
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` が通っている
- [ ] `cargo test --workspace` が通っている
- [ ] 抽象構文木検査（AST lint）がある場合は通っている
- [ ] `/self-review` が完了している
```

## 4. User Review

ユーザー確認が必要な場合は、final verification の前に独立したタスクグループとして置きます。
画面確認ではなく、CLI 出力、生成 artifact、reference image、比較結果など、kcf の成果物で確認できる形にします。

```markdown
---

## x. User Review

> ユーザーから受けた指摘は `[/]` で閉じる。通常の開発タスク `[x]` と混ぜない。

- [ ] x.1 実装結果と検証結果をユーザーに提示する
- [ ] x.2 フィードバックを本 `tasks.md` に追記し、対応済みを `[/]` にする
```

## 5. Final Verification

最後のタスクグループは必ず置きます。

```markdown
---

## x. Final Verification

- [ ] x.1 `/lint-and-ast-lint` を実行し、静的検査（lint）と抽象構文木検査（AST lint）の結果を記録する
- [ ] x.2 `/self-review` を実行し、差分範囲の設計、テスト、検証の妥当性を確認する
- [ ] x.3 `npx -y @fission-ai/openspec validate "<change-id>" --strict` を実行する
- [ ] x.4 PR 作成が必要な場合は `/create_pull_request` を使う
- [ ] x.5 統合後に `/openspec-archive-change` を実行する
```

## 6. 分割ルール

タスクグループが大きい場合は、責務で分割します。

- 公開 API
- renderer backend
- exporter backend
- CLI
- vendor bundle / checksum
- test harness
- quality gate

同じファイルを複数作業者で同時に触らせません。
