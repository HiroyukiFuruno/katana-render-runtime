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

## 7. PR レビュー接続条件

Self-review の PASS は、外部 cloud review の完了、Ready 化、または merge 準備完了の代替ではありません。Self-review は Draft PR 作成へ進む前提条件としてのみ扱います。

Draft PR 作成後は、必ず次の順序で後続工程へ接続します。cloud review の依頼コメントには、対象 HEAD と current PR bodyを追跡できるよう `krr-review phase=<initial|final> head=<40 lowercase hex> body-sha256=<64 lowercase hex>` のmarkerを記録します。bodyはstringであることを確認し、NULまたはlone surrogateを含む場合、またはUTF-8 strict encodeに失敗する場合はdigest計算前にfail-closedで停止します。正常なbodyはGitHub APIから取得した文字列を正規化せずUTF-8 bytesとしてSHA-256化します。

1. Draft PR の依頼直前にGitHub APIからcurrentのHEAD/bodyを再取得し、body digest付き`phase=initial` markerとともに初回 cloud review を依頼する。
2. 指摘を取得・分類し、分離可能な指摘の修正を subagent へ委譲する。
3. 各指摘の修正を確認し、review thread へ reply して resolve する。
4. 修正の push または同一 HEAD の PR本文編集後は、旧 marker・review・trusted success を無効として扱う。GitHub APIから current HEAD/body を再取得し、strict検証とdigest計算をやり直して、新しい`phase=initial` markerによる cloud review を**完了**させる。initial review を飛ばして final review へ進まない。
5. 指摘がなかった場合、または 4 の current HEAD/body に束縛した initial review 完了後に全指摘の reply / resolve と検証が済んだ場合だけ、依頼直前に再取得した同一HEAD/bodyに対してbody digest付き`phase=final` markerで最終 cloud reviewを依頼する。取得値が latest initial marker と異なる場合は final を投稿せず、4 の新しい initial review からやり直す。
6. `pr-ready-check` で review 完了、未 resolve thread 0、CI / DoD PASS を機械確認する。

initial / final のいずれで新規指摘が出ても、指摘を subagent へ修正委譲し、修正確認、push、review thread への reply、resolve を行います。修正の push または PR本文編集は必ず current HEAD/body の strict initial review を完了させてから final review へ戻ります。PR bodyを編集した場合は同じHEADでも旧markerと旧reviewを失効させ、initial marker→bot review→final marker→bot reviewをやり直します。self-reviewの引き渡し確認ではmarkerのHEAD/body digestとtrusted evidenceのHEAD/external_idが同一境界に一致することを確認します。trusted Check Run evidenceの`pr_body_sha256`は、current PR bodyをstrict UTF-8 SHA-256した値と一致する64桁小文字hexが**ちょうど1個**だけ存在しなければならず、missing、duplicate、stale、または異なるdigestはfail-closedで拒否します。

CI green だけでレビュー完了、Ready 化、または merge 準備完了とは扱いません。

## no-issues 証跡の共通契約

`Reviewed commit` の prefix は current HEAD に一致させます。

review botの「no issues」はformal reviewではなく、trusted botがPR Issueへ投稿したcanonical commentだけを完了証跡として受理します。本文は `Codex Review: Didn't find any major issues...` または同じcanonical prefixの短文に続き、`**Reviewed commit:** \`<10〜40桁の小文字SHA prefix>\`` を含み、current HEADがそのprefixで始まらなければなりません。`created_at == updated_at` を必須とし、phase marker間のcanonical候補は高々1件、duplicateはfail-closedで拒否します。任意のdetailsは省略またはcanonicalな1つだけ（summary `ℹ️ About Codex in GitHub`、本文8192文字以内）を許可し、nested details、details外のclosing/sentinel文字列、重複canonical行は拒否します。reactionや任意のbot commentは証跡にしません。finalのno-issues証跡は参照Issueの最終`updated_at`より後でなければなりません。Codexが同一HEADの同一結果をduplicate suppressionした場合だけ、initial/final markerが同じHEAD・本文digestで、unresolved threadが0、かつIssue更新後かつfinal marker前に記録されたcanonical commentをinitial-to-final evidenceとして再利用できます。通常のformal reviewまたは指摘対応経路では、final marker後に別のreview完了証跡を取得します。

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
