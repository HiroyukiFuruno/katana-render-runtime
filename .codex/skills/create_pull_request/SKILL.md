---
name: create-pull-request
description: katana-diagram-renderer の Pull Request を Draft で作成し、Codex review と指摘対応、品質ゲートを完了してから Ready 化する。base branch を文脈から確認し、PR 本文に検証結果を含める。
---

# Create Pull Request

PR 作成前に、差分、検証、base branch を確認します。PR は必ず Draft で作成し、review と指摘対応を完了してから Ready にします。
推測で `master` や `main` を選びません。

governance bootstrap と保護mergeは、PR外の絶対path `/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py` の `prepare` / `activate` / `merge` / `finalize` / `verify` に限定する。`prepare --apply` は保存済みの人間用 `gh auth` でexpected default branchを確認してApp-only rulesetを先行作成するためJWT不要、非mutating `merge` dry-runも保存済み認証によるpublic readだけを使う。`activate --apply` はそのrulesetと固定証跡のverify-only、`merge --apply` はApp-only merge、`finalize --apply` はmerge後の再読、`verify` はsmoke確認とする。activate/merge/finalize/verifyの各実行直前にfreshな `KRR_GOVERNANCE_APP_JWT` だけを環境へ設定し、script自身がexact KRR repository・operation-specific least-privilegeで短期IATをmintする。IAT responseのidentity・expiry・scope・permissionsをstrict検証し、JWT/private key/IATをCLI引数・出力へ出さない。`--expected-base --expected-head --expected-app-id --expected-diff-sha256`、expected default branch、完全な `--allowed-workflow`、current body digest/review/thread/Issue/CI/trusted/latch、ruleset exact App bindingを固定し、merge APIにはexpected head SHAを渡して成功後にmerge commit/rulesetを再読する。人間/admin/UI/通常のGitHub CLI merge（`gh pr merge`を含む）は禁止し、PR checkoutのコードをevidenceとして実行しない。通常の `pr-ready-check` は緩和しない。

## 1. 前提確認

```bash
git status --short
git branch --show-current
git branch -a
```

- commit 済みである。
- `/self-review` が完了している。
- `/lint-and-ast-lint` で必要な検証が通っている。
- 未追跡や他者差分を混ぜていない。

## 2. base branch を決める

1. ユーザーが明示した base があればそれを使う。
2. OpenSpec の task branch なら、対応する integration branch を base にする。
3. integration branch 自体なら、通常は repository default branch を base にする。
4. 判断できない場合は、候補と理由を示してユーザーに確認する。

base branch の存在を確認します。

```bash
git branch -a | rg "<base-branch>"
```

## 3. PR template を確認する

```bash
test -f .github/PULL_REQUEST_TEMPLATE.md
```

template があれば優先します。
なければ次の形で本文を作ります。

```markdown
<!-- 日本語でレビューしてください。 -->

## 概要

## 対応内容

## 影響範囲

## 動作確認
```

## 4. Draft PR を作る

Draft PR を作る前に、branch の全 commit が参照する同一 repository の Issue 集合を収集します。PR 本文の closing Issue 集合は commit 参照 Issue 集合と完全一致させ、不足も余分も許可しません。各 Issue には GitHub closing keyword（`Closes #N`、`Fixes #N`、`Resolves #N`、または同一 repository の完全な Issue URL）による closing reference を含めます。`Refs #N` だけでは不十分です。

```bash
gh pr create --draft --base "<base-branch>" --head "<current-branch>" --title "<title>" --body-file "<body-file>"
```

`--base` は必須です。

作成直後に Draft 状態を機械確認します。`isDraft=true` でなければ、以降へ進みません。

```bash
gh pr view "<pr-number>" --json isDraft --jq '.isDraft'
```

## 5. Draft 上で初回 review と指摘対応

Draft のまま、次の順序を厳守します。

1. 依頼直前に GitHub API から current PR の `headRefOid` と `body` を再取得する。body は string 以外、NUL、lone surrogate、UTF-8 strict 不能を fail-closed で拒否し、空文字列への正規化をしない。受理したbody文字列だけを正規化せずUTF-8 bytesとしてSHA-256し、次の marker と `@codex review` を同じコメント本文にこの順で含めて初回 review を依頼する。markerのSHA値はlowercase hexでなければならない。

   ```bash
   pr_json="$(gh api "repos/<owner>/<repo>/pulls/<pr-number>")"
   head_sha="$(jq -r '.head.sha' <<<"$pr_json" | tr '[:upper:]' '[:lower:]')"
   body_sha256="$(python3 -c '
import hashlib, json, sys
value = json.load(sys.stdin)
body = value.get("body")
if not isinstance(body, str) or "\x00" in body or any(0xD800 <= ord(char) <= 0xDFFF for char in body):
    raise SystemExit("current PR body is not valid text")
try:
    print(hashlib.sha256(body.encode("utf-8", "strict")).hexdigest())
except UnicodeEncodeError:
    raise SystemExit("current PR body is not strict UTF-8")
' <<<"$pr_json")" || exit 1
   gh pr comment "<pr-number>" --body "<!-- krr-review phase=initial head=${head_sha} body-sha256=${body_sha256} -->
   @codex review"
   ```

2. review、review thread、PR コメントをすべて取得し、P0/P1/その他、対応要否、担当を分類する。
3. 修正が必要な指摘は、対象ファイルと DoD を明示して修正担当 subagent へ移譲する。同じファイル・責務を複数担当に重ねない。
4. P0/P1 に限らず対応対象と判断した通常指摘も、修正担当 subagent が **修正 → ローカル検証 → push → 該当 thread への reply → resolve** の順で完了させる。CI が green でも、未resolve の指摘があれば完了扱いにしない。
5. 最終 review は、pushまたはPR本文編集が発生した場合に限り、まず current HEAD/body digestで新しい initial marker付きcloud reviewを完了してから実施する。依頼直前にGitHub APIからcurrent PRの`headRefOid`と`body`を再取得し、push/body変更前のinitial/final markerとreview証跡を無効化する。旧initialと新finalだけを組み合わせる経路は禁止する。HEAD/bodyが不変でpushもbody編集もない場合だけ、既存のinitialからfinalへ進める。最終reviewのmarkerと`@codex review`は同じコメント本文にこの順で含める。

   ```bash
   pr_json="$(gh api "repos/<owner>/<repo>/pulls/<pr-number>")"
   head_sha="$(jq -r '.head.sha' <<<"$pr_json" | tr '[:upper:]' '[:lower:]')"
   body_sha256="$(python3 -c '
import hashlib, json, sys
value = json.load(sys.stdin)
body = value.get("body")
if not isinstance(body, str) or "\x00" in body or any(0xD800 <= ord(char) <= 0xDFFF for char in body):
    raise SystemExit("current PR body is not valid text")
try:
    print(hashlib.sha256(body.encode("utf-8", "strict")).hexdigest())
except UnicodeEncodeError:
    raise SystemExit("current PR body is not strict UTF-8")
' <<<"$pr_json")" || exit 1
   gh pr comment "<pr-number>" --body "<!-- krr-review phase=final head=${head_sha} body-sha256=${body_sha256} -->
   @codex review"
   ```

6. 最終 review で新規指摘が出た場合は、指摘ごとに修正担当 subagent へ移譲し、修正・検証、push、該当 thread への reply、resolve を完了する。その後は必ず current HEAD/body digestで新しい initial marker付きcloud reviewを依頼・完了し、次に同じdigestのfinal marker付きreviewを依頼する。PR bodyを編集した場合も同じ扱いとし、未resolve 0かつ新規指摘なしになるまで反復する。旧initialと新finalの組み合わせ、またはpush/body変更後にinitialを省略する経路は禁止する。

初回・最終 review とも、コメント投稿だけでなく結果を取得して確認します。レビュー取得では review thread を省略せず、GraphQL の `pageInfo.hasNextPage` が `false` になるまで `pageInfo.endCursor` を次の `after` cursor に渡して全ページ取得します。必要に応じて次を使います。

```bash
gh pr view "<pr-number>" --json headRefOid,comments,isDraft,statusCheckRollup
gh api graphql \
  -f query='query($owner:String!, $repo:String!, $number:Int!, $reviewCursor:String, $threadCursor:String, $commentCursor:String) { repository(owner:$owner, name:$repo) { pullRequest(number:$number) { reviews(first:100, after:$reviewCursor) { nodes { id body state author { login } commit { oid } submittedAt } pageInfo { hasNextPage endCursor } } reviewThreads(first:100, after:$threadCursor) { nodes { id isResolved comments(first:50, after:$commentCursor) { nodes { id body author { login } } pageInfo { hasNextPage endCursor } } } pageInfo { hasNextPage endCursor } } } } }' \
  -f owner="<owner>" -f repo="<repo>" -F number="<pr-number>" -f reviewCursor="<review-cursor-or-null>" -f threadCursor="<thread-cursor-or-null>" -f commentCursor="<comment-cursor-or-null>"
```

レビューは `gh pr view` のboundedな一覧だけに依存せず、GraphQL `reviews` connection を `pageInfo.hasNextPage=false` になるまで取得します。各 `reviewThreads` connection も同様に全ページ取得し、各nodeのnested `comments` connectionについても `pageInfo.hasNextPage=true` なら `endCursor` を `commentCursor` に渡した追加取得を行います。nested comments の追加取得に失敗・省略した場合は fail-closed（Ready化不可）とします。各cursorの初回値は null とし、返された `endCursor` を同じconnectionの次のcursorへ設定します。canonical gateを使う場合も、全ページ取得を実施した結果で指摘・未resolve thread数を判定します。

最終 review 後、Ready 化前に次を機械確認します。

- 最終 review が最新 head (`headRefOid`) を対象に完了している。
- 未resolve thread が 0 件である。
- CI が green である。
- self-review、lint、テスト、coverage、OpenSpec/DoD がすべて PASS である。
- 各 Issue の `non-Draft target` は 256 件以下である（256 non-Draft target invariant）。超過した場合は bypass せず、影響する PR を Draft に戻すか closing reference を外してから merge 前に解消します。
- `pr-ready-check` は最初に、参照IssueがOPENであること、依存更新証跡が揃っていること、PR rangeのIssue契約が完全一致することを検査します。
- trusted Check Run evidence の `pr_body_sha256` は、current PR bodyを正規化せず strict UTF-8 SHA-256した値と一致する64桁小文字hexがちょうど1個だけ存在しなければなりません。missing、duplicate、stale、または異なるdigestはfail-closedで拒否します。

### Governance workflow の初回導入・改修

対象PRが governance workflow 自体の初回導入または改修である場合だけ、PR checkout外の専用skill scriptを次の順序で実行します。通常PRへ例外を拡張せず、PR内の条件分岐・ブランチ名・Issue番号・PR author・workflow変更を自己承認の根拠にしません。

1. `prepare --apply`: 保存済みの人間用 `gh auth` でexpected default branchを確認し、App-only active rulesetを先行作成する。fresh JWTは不要です。
2. `activate --apply`: fresh JWTだけを環境変数へ設定し、scriptがexact repository・operation-specific least-privilegeでmintした短期IATのresponse identity/expiry/scope/permissionsをstrict検証して、固定base/head/diff/allowlistと一時Check Runをverify-only確認する。
3. `pr-ready-check`成功後にReady化し、ユーザーのfresh merge承認を取得する。merge直前に同じ`pr-ready-check`を再実行し、global skillの`merge --apply`でactive rulesetのexact Integration App bypassとclassic branch protectionのrequired conversation resolution、phase-exact trusted/latch、最新body digest/review/thread/Issue/CI、Contents write IAT、merge API expected SHAを再検証して保護mergeする。
4. `finalize --apply`: merge後にmerge commit、ruleset、classic protectionを再取得し、永続required contextsを確認する。
5. 同じApp-only merge routeで使い捨てsmoke PRをmergeし、別のfresh JWTからread-only IATをmintして`verify`でsmoke結果を確認する。

active rulesetは更新操作専用であり、exact Integration Appのbypass actor（`bypass_mode=pull_request`）だけを許可し、人間/admin/UI/通常のGitHub CLIによるprotected default branchの更新・mergeとAppによる直接ref更新を拒否します。required checkとconversationの実施主体はclassic branch protectionに置きます。classic branch protectionは`enforce_admins=true`、`required_conversation_resolution=true`、`strict=true`を維持し、required contextsをフェーズごとにApp bindingします。bootstrap中は`KRR / PR governance bootstrap`を専用App ID、finalize後は`KRR / PR governance (trusted check)`を専用App ID、`KRR / PR governance review latch`をGitHub Actions App ID `15368`に固定します。dry-runはmutation 0を証明します。人間/admin/UI/通常のGitHub CLI merge（`gh pr merge`を含む）は禁止し、dynamic barrierはdefense-in-depthであってcold-stateのmerge authorityではありません。各`--apply`直前にfresh confirmationを取得し、prepare以外はfresh JWTのみを受け取ります。JWT/private key/IATをargv・出力へ出さず、rollbackでもrulesetを外しません。

## no-issues の Issue comment 証跡

`Reviewed commit` の prefix は current HEAD に一致させます。

review botの「no issues」はformal reviewではなく、trusted botがPR Issueへ投稿したcanonical commentだけを完了証跡として受理します。本文は `Codex Review: Didn't find any major issues...` または同じcanonical prefixの短文に続き、`**Reviewed commit:** \`<10〜40桁の小文字SHA prefix>\`` を含み、current HEADがそのprefixで始まらなければなりません。`created_at == updated_at` を必須とし、phase marker間のcanonical候補は高々1件、duplicateはfail-closedで拒否します。任意のdetailsは省略またはcanonicalな1つだけ（summary `ℹ️ About Codex in GitHub`、本文8192文字以内）を許可し、nested details、details外のclosing/sentinel文字列、重複canonical行は拒否します。reactionや任意のbot commentは証跡にしません。finalのno-issues証跡は参照Issueの最終`updated_at`より後でなければなりません。Codexが同一HEADの同一結果をduplicate suppressionした場合だけ、initial/final markerが同じHEAD・本文digestで、unresolved threadが0、かつIssue更新後かつfinal marker前に記録されたcanonical commentをinitial-to-final evidenceとして再利用できます。通常のformal reviewまたは指摘対応経路では、final marker後に別のreview完了証跡を取得します。

## 6. Ready 化と承認依頼

上記の review 完了・未resolve 0・CI/DoD PASS を確認した後だけ、Draft を Ready にします。

```bash
just pr-ready-check "<pr-number>" && \
  gh pr ready "<pr-number>"
gh pr view "<pr-number>" --json isDraft --jq '.isDraft'
```

確認結果が `false` になった後に限り、fresh merge 承認を依頼します。承認後はglobal skillの`merge --apply`直前に同じ `just pr-ready-check "<pr-number>"` を再実行し、Ready PRの最新Issue/marker/thread/CI/base/head/body digestとtrusted/latchを再検証してからApp-only mergeします。Ready 化前に承認を依頼したり、CI green だけを理由に review 完了と扱ったりしません。

## 7. Ready 後確認

```bash
gh pr view "<pr-number>" --web
gh pr checks "<pr-number>"
```

CI が失敗した場合は、`gh-fix-ci` 相当の調査に進みます。

## 報告

- PR URL
- base/head
- 初回・最終 review の完了結果
- 未resolve thread 数（0 件）
- self-review、lint、テスト、coverage、OpenSpec/DoD の検証結果
- CI 状態
