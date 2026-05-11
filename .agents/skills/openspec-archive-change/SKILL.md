---
name: openspec-archive-change
description: 実装、検証、PR 統合が完了した katana-canvas-forge の OpenSpec change を archive へ移す。未完了タスクが残る変更では使わない。
---

# OpenSpec Archive Change

完了した OpenSpec change だけを archive へ移します。
archive は「作業を終えた記録」であり、未完了の作業を隠すために使いません。

## 実行入口

専用入口がなければ、リポジトリルートから次を使います。

```bash
npx -y @fission-ai/openspec <command>
```

## 事前条件

- `/openspec-verify-change` が PASS している。
- `tasks.md` の実装タスクがすべて `[x]` になっている。
- 必要な品質ゲートが通っている。
- PR 統合が必要な通常変更では、統合済みである。
- `release/vX.Y.Z` ブランチで release PR に archive 移動を含める場合は、PR 作成前の archive を許可する。

## 手順

1. change を選ぶ。

   ```bash
   npx -y @fission-ai/openspec list --json
   ```

   指定がなく、active change が複数ある場合はユーザーに確認します。

2. 状態を確認する。

   ```bash
   npx -y @fission-ai/openspec status --change "<change-id>" --json
   ```

3. `tasks.md` を読み、未完了がないことを確認する。

   未完了 `- [ ]` がある場合は停止します。

4. 仕様差分がある場合は、main spec へ反映済みか確認する。

   反映が必要なら、archive の前にユーザーへ確認します。

5. archive 先が存在しないことを確認する。

   ```bash
   test ! -e "openspec/changes/archive/$(date +%F)-<change-id>"
   ```

6. archive へ移す。

   ```bash
   mkdir -p openspec/changes/archive
   mv "openspec/changes/<change-id>" "openspec/changes/archive/$(date +%F)-<change-id>"
   ```

7. 検証する。

   ```bash
   npx -y @fission-ai/openspec validate --strict
   ```

## 報告形式

- archived path
- 事前に確認した検証結果
- main spec への反映有無
- 未解決事項の有無

## 禁止

- 未完了タスクを archive で隠さない。
- 通常変更では PR 統合前提の変更を未統合のまま archive しない。
- release PR では、実装・検証が完了した change を PR 作成前に archive へ移動する。
- ユーザーの承認なしに強制 archive しない。
