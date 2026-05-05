---
name: openspec-verify-change
description: katana-canvas-forge の実装が OpenSpec の提案、設計、仕様差分、tasks.md と一致しているか確認する。archive や PR 前の最終確認で使う。
---

# OpenSpec Verify Change

実装が OpenSpec と一致しているかを、完了性、正しさ、整合性の 3 点で確認します。

## 実行入口

専用入口がなければ、リポジトリルートから次を使います。

```bash
npx -y @fission-ai/openspec <command>
```

## 手順

1. change を選ぶ。

   指定がない場合は active change を確認し、複数あればユーザーに確認します。

   ```bash
   npx -y @fission-ai/openspec list --json
   ```

2. 状態を確認する。

   ```bash
   npx -y @fission-ai/openspec status --change "<change-id>" --json
   ```

3. context files を取得する。

   ```bash
   npx -y @fission-ai/openspec instructions apply --change "<change-id>" --json
   ```

4. artifact を読む。

   - `proposal.md`
   - `design.md`
   - `specs/**/spec.md`
   - `tasks.md`

5. 完了性を確認する。

   - `tasks.md` に未完了 `- [ ]` がないか。
   - 仕様差分の requirement が実装されているか。
   - scenario に対応するテストがあるか。

6. 正しさを確認する。

   - 公開 API の型と責務が proposal/design と一致するか。
   - renderer/exporter の失敗経路がテストされているか。
   - Mermaid/Draw.io/vendor/checksum/version pinning が仕様どおりか。
   - CLI が library の薄い利用者に留まっているか。

7. 整合性を確認する。

   - crate の責務が分かれているか。
   - 新しい例外、fallback、allow、ignore が仕様化されているか。
   - UI、翻訳、アイコン、release など kcf 外の関心が混ざっていないか。

8. 品質ゲートを確認する。

   - `/lint-and-ast-lint` の結果
   - `/self-review` の結果
   - 必要な `cargo test --workspace`

## 報告形式

```markdown
# OpenSpec Verify: <change-id>

## 結論
PASS / FAIL

## 未完了
- なし / 対応が必要な項目

## 根拠
- 読んだ artifact
- 確認した実装ファイル
- 実行した検証コマンド

## 指摘
- 重大度、対象ファイル、修正方針
```

重大な未実装や未完了タスクがある場合、archive に進めません。
