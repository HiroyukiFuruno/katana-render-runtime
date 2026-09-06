---
name: impl-release
description: katana-diagram-renderer で指定バージョンの実装、品質確認、release branch PR 作成、自動リリース確認までを一気通貫で進めるときに使う。/impl-release vX.Y.Z と同等のリリース実装ワークフロー。
---

# impl-release

`/impl-release vX.Y.Z` として扱う、katana-diagram-renderer のリリース実装入口です。
この repository は `release/vX.Y.Z` から `master` へ取り込み依頼（Pull Request）を作り、merge 後に自動リリースします。
初回公開版は `v0.1.0` から開始します。

governance bootstrap と保護mergeは、PR外の絶対path `/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py` の `prepare` / `activate` / `merge` / `finalize` / `verify` に限定する。`prepare --apply` は保存済みの人間用 `gh auth` でexpected default branchを確認してApp-only rulesetを先行作成するためJWT不要、非mutating `merge` dry-runも保存済み認証によるpublic readだけを使う。`activate --apply` はそのrulesetと固定証跡のverify-only、`merge --apply` はApp-only merge、`finalize --apply` はmerge後の再読、`verify` はsmoke確認とする。activate/merge/finalize/verifyの各実行直前にfreshな `KRR_GOVERNANCE_APP_JWT` だけを環境へ設定し、script自身がexact KRR repository・operation-specific least-privilegeで短期IATをmintする。IAT responseのidentity・expiry・scope・permissionsをstrict検証し、JWT/private key/IATをCLI引数・出力へ出さない。`--expected-base --expected-head --expected-app-id --expected-diff-sha256`、expected default branch、完全な `--allowed-workflow`、current body digest/review/thread/Issue/CI/trusted/latch、ruleset exact App bindingを固定し、merge APIにはexpected head SHAを渡して成功後にmerge commit/rulesetを再読する。人間/admin/UI/通常のGitHub CLI merge（`gh pr merge`を含む）は禁止し、PR checkoutのコードをevidenceとして実行しない。通常の `pr-ready-check` は緩和しない。

## 実行ルール

1. ユーザー指定の version を対象にする。例: `v0.1.0`
2. 作業開始前に `git status --short --branch` と `git fetch origin --prune --tags` を実行する。
3. 既存差分がある場合、release 作業へ混ぜる前に関心事を分ける。
4. 作業ブランチは `release/vX.Y.Z` に統一する。
5. 直接 `cargo publish` や tag 作成で迂回しない。公開は merge 後の自動実行基盤（GitHub Actions）に任せる。
6. 秘匿値（secret）は `CARGO_REGISTRY_TOKEN` を使う。値の取得や登録はユーザーが行う。
7. 不自然な version 飛び番は停止し、`just VERSION=vX.Y.Z release-target-check` の結果を確認する。

## Phase 1: 準備

```bash
git switch master
git pull --ff-only origin master
just VERSION=vX.Y.Z release-target-check
git switch -c release/vX.Y.Z
```

対象 version の OpenSpec change や tasks がある場合は、先に読みます。
見つからない場合は、release 内容を差分と `docs/release.md` から確認します。

対象 version より前の完了済み OpenSpec change は、`release-check` と `pre-pr` の前に archive へ移動します。archive の変更も release の正式な commit に含め、merge 後まで先送りしません。未完了の change は完了条件を満たすまで archive せず、対象 version の release gate を通してはいけません。

## Phase 2: 実装と検証

未完了 task を実装し、必要に応じて `tasks.md` を更新します。
実装後は次を通します。

```bash
just check
just VERSION=vX.Y.Z release-check
git diff --check
```

失敗した場合は、除外や allow で逃げず、設計またはテストを直して同じ gate に戻ります。

## Phase 3: commit と push

`lefthook` を通すため、通常の commit / push を使います。commit 前に、対象変更に対応する同一 repository の canonical な OPEN Issue を選び、Issue 番号が正整数であることを確認します。以後の各 commit メッセージには、その Issue への `Refs #${issue_number}` を必ず含めます。Issue の選択・番号確認ができない場合は commit しません。

```bash
git status --short --branch
git add <release に必要な files>
git commit -m "release: vX.Y.Z リリース準備 Refs #${issue_number}"
git push -u origin release/vX.Y.Z
```

`git push --no-verify` は使いません。

## Phase 4: Draft PR 作成と cloud review

`release/vX.Y.Z` から `master` へ Draft の Pull Request を作成します。対象 version 以前の完了済み OpenSpec change が archive 済みであることを確認してから、PR 前の gate と Draft PR 作成へ進みます。

```bash
lefthook run pre-pr
pr_url="$(gh pr create --draft --base master --head release/vX.Y.Z --title "Prepare vX.Y.Z release" --body-file <pr-body-file>)"
gh pr view "${pr_url}" --json isDraft --jq '.isDraft'
review_body="$(gh pr view "${pr_url}" --json headRefOid,body | python3 -c '
import hashlib, json, re, sys
value = json.load(sys.stdin); head = value.get("headRefOid"); body = value.get("body")
if not isinstance(head, str) or re.fullmatch(r"[0-9a-f]{40}", head) is None or not isinstance(body, str) or "\0" in body:
    raise SystemExit("Current PR head/body is invalid")
try:
    digest = hashlib.sha256(body.encode("utf-8", "strict")).hexdigest()
except UnicodeEncodeError:
    raise SystemExit("Current PR body is not strict UTF-8")
print(f"<!-- krr-review phase=initial head={head} body-sha256={digest} -->\n@codex review")
')"
gh pr comment "${pr_url}" --body "${review_body}"
```

`gh pr view` の結果が `true` であることを確認してから、Draft PR 上で明示的に初回 `@codex review` を依頼します。
初回 marker の投稿直前に GitHub API から current PR の HEAD と本文を再取得する。marker は `^<!-- krr-review phase=(?:initial|final) head=[0-9a-f]{40} body-sha256=[0-9a-f]{64} -->$` に完全一致させ、本文は JSON string・NULなし・UTF-8 strict を確認したバイト列の SHA-256 小文字16進表現だけを使う。
レビュー（review）はローカルの自己レビューではなく cloud review を正とし、指摘は GitHub 上の review comment から取得して対応します。

## Phase 5: PR gate

Draft 上の cloud review は最低2回、かつ最新 HEAD・本文 digest に対して実施します。

1. 初回 review: PR 作成直後に `@codex review` を投稿する。
2. 最終 review: 指摘対応、thread reply / resolve、検証を終えた後（指摘がなく修正 push がない場合も merge 前）、同じ PR の最新 HEAD・本文に対してもう一度 `@codex review` を投稿する。

指摘修正は、指摘対象ファイルと責務を分離して subagent へ移譲します。各 review thread について、修正内容を thread へ reply し、確認後に resolve します。レビュー証跡の有効性は current HEAD と current PR body digest に固定します。push 後は旧 HEAD の marker・review・trusted success を無効化し、GitHub API で current HEAD/body を再取得して新しい `initial` marker と cloud review を取得してから final review へ進みます。同じ HEAD でも PR本文を編集した場合は旧 body digest の marker・review・trusted success を無効化し、同じ手順をやり直します。push と本文編集のどちらも無い場合に限り、同一HEAD・同一body digest・Issue freshness・未resolve 0 の条件を満たす既存証跡を再利用できます。
完了条件は「最低2回実施」「最新 HEAD・本文 digest が review 済み」「未 resolve thread が 0」です。

最終 review の marker投稿直前に、GitHub API から current PR の HEAD・本文を再取得し、initial marker と同じstrict検証とdigest計算を行う。取得値が initial marker と異なる場合は final marker を投稿せず、新しい initial review からやり直す。

```bash
review_body="$(gh pr view "${pr_url}" --json headRefOid,body | python3 -c '
import hashlib, json, re, sys
value = json.load(sys.stdin); head = value.get("headRefOid"); body = value.get("body")
if not isinstance(head, str) or re.fullmatch(r"[0-9a-f]{40}", head) is None or not isinstance(body, str) or "\0" in body:
    raise SystemExit("Current PR head/body is invalid")
try:
    digest = hashlib.sha256(body.encode("utf-8", "strict")).hexdigest()
except UnicodeEncodeError:
    raise SystemExit("Current PR body is not strict UTF-8")
print(f"<!-- krr-review phase=final head={head} body-sha256={digest} -->\n@codex review")
')"
gh pr comment "${pr_url}" --body "${review_body}"
```

最終 review コメントには strict final marker を含め、その直後に `@codex review` を置きます。以後、別の push、または同一 HEAD での本文編集を行った場合は旧 review を無効として扱い、current HEAD・本文の initial marker から review をやり直します。

次を確認します。

- `Test and Build (macos-latest)`
- `Test and Build (ubuntu-latest)`
- `Test and Build (windows-latest)`
- `preflight`
- `just VERSION=vX.Y.Z release-target-check`
- OpenSpec の tasks / DoD
- cloud review の未 resolve thread が 0 であること

```bash
gh pr checks --watch "${pr_url}"
just VERSION=vX.Y.Z release-target-check
```

CI green だけでは Ready または merge の条件を満たしません。DoD、release-target gate、最新 HEAD・本文 digest の review、未 resolve 0 をすべて確認します。

## Phase 6: Ready 化と merge 承認

上記の全 gate が通った後、次の専用チェックを実行します。

```bash
just pr-ready-check "<number>" && gh pr ready "${pr_url}"
```

`pr-ready-check` は参照Issueが OPEN であること、依存更新証跡が揃っていること、PR range の Issue contract が完全一致すること（不足・余分を含む）を先に検証します。さらに latest marker とpost-marker bot review、writer/trusted Check Run evidence の current `pr_body_sha256` 完全一致を要求します。trusted Check Run の query 結果には、current PR bodyを正規化せず strict UTF-8 SHA-256した64桁小文字hexと完全一致する `pr_body_sha256` が **ちょうど1個（exactly one）** だけ存在しなければならず、missing、duplicate、stale、または不一致は fail-closed で拒否します。成功するまで `gh pr ready` は実行しません。Ready 化後に、ユーザーへ merge 承認を求めます。承認後、global bootstrap skillの `merge --apply` 直前に同じ `just pr-ready-check "<number>"` を再実行し、Ready PRの最新Issue/marker/thread/CI/base/head/body digestとtrusted digest bindingを再検証してから、PR checkout外のApp-only mergeを実行します。

`pr-ready-check` の前に、GitHub の review thread を全ページ取得して未 resolve が 0 件であることを確認します。未 resolve thread が 1 件でも残っている場合は Ready 化せず、対象 subagent に修正・reply・resolve を戻します。

### governance workflow の初回 bootstrap

PR が `.github/workflows/` の governance workflow 自体を追加・変更する初回 bootstrap に限り、専用skill scriptをPR checkout外で次の順序に固定します。通常PRへ例外を拡張せず、PR内のworkflow、branch名、Issue、Check Runを自己承認の根拠にしません。

1. `prepare --apply`: 保存済みの人間用 `gh auth` でexpected default branchを確認し、App-only active rulesetを先行作成する。fresh JWTは不要です。
2. `activate --apply`: fresh JWTを環境変数に設定し、scriptがexact repository・operation-specific least-privilegeでmintした短期IATのresponse identity/expiry/scope/permissionsをstrict検証したうえで、一時Check Runと固定base/head/diff/allowlistをverify-only確認する。
3. Ready化後、ユーザーのfresh merge承認を取得し、同じ`pr-ready-check`を直前に再実行してから`merge --apply`を実行する。mergeはactive rulesetのexact Integration App bypassとclassic branch protectionのrequired conversation resolution、phase-exact trusted/latch、最新body digest/review/thread/Issue/CIを再検証し、Contents writeを含む短期IATとmerge API expected SHAを検証してから行う。
4. `finalize --apply`: merge後にmerge commit、ruleset、classic protectionを再取得し、永続required contextsを確認する。
5. 同じApp-only merge routeで使い捨てsmoke PRをmergeし、別のfresh JWTからread-only IATをmintして`verify`でsmoke結果を確認する。

active rulesetは更新操作専用であり、exact Integration Appのbypass actor（`bypass_mode=pull_request`）だけを許可し、人間/admin/UI/通常のGitHub CLIによるprotected default branchの更新・mergeとAppによる直接ref更新を拒否します。required checkとconversationの実施主体はclassic branch protectionに置きます。classic branch protectionは`enforce_admins=true`、`required_conversation_resolution=true`、`strict=true`を維持し、required contextsをフェーズごとにApp bindingします。bootstrap中は`KRR / PR governance bootstrap`を専用App ID、finalize後は`KRR / PR governance (trusted check)`を専用App ID、`KRR / PR governance review latch`をGitHub Actions App ID `15368`に固定します。dry-runはmutation 0を証明します。人間/admin/UI/通常のGitHub CLI merge（`gh pr merge`を含む）は禁止し、dynamic barrierはdefense-in-depthであってcold-stateのmerge authorityではありません。各`--apply`の直前にfresh confirmationを取得し、prepare以外の各コマンドではfresh JWTのみを環境へ設定します。JWT/private key/IATをargv・出力へ出さず、rollbackでもrulesetを外しません。

## Phase 7: merge と自動リリース

承認後は、直前の `just pr-ready-check "<number>"` が成功した場合だけ merge します。merge 後は、OpenSpec change が release 前に archive 済みであることを確認し、Release workflow の結果を確認します。

```bash
SCRIPT=/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py
python3 "$SCRIPT" merge \
  --repository <owner>/<repo> --pr "<number>" \
  --expected-base <base-sha> --expected-head <head-sha> \
  --expected-app-id <governance-app-id> \
  --allowed-workflow <every-changed-workflow-path> \
  --expected-diff-sha256 <binary-full-index-diff-digest> --apply
# OpenSpec change は release gate 前に archive 済みであることを確認
```

merge 後、Release workflow と crates.io 公開結果を確認します。

```bash
gh run list --workflow Release --limit 5
```

## no-issues の Issue comment 証跡

`Reviewed commit` の prefix は current HEAD に一致させます。

review botの「no issues」はformal reviewではなく、trusted botがPR Issueへ投稿したcanonical commentだけを完了証跡として受理します。本文は `Codex Review: Didn't find any major issues...` または同じcanonical prefixの短文に続き、`**Reviewed commit:** \`<10〜40桁の小文字SHA prefix>\`` を含み、current HEADがそのprefixで始まらなければなりません。`created_at == updated_at` を必須とし、phase marker間のcanonical候補は高々1件、duplicateはfail-closedで拒否します。任意のdetailsは省略またはcanonicalな1つだけ（summary `ℹ️ About Codex in GitHub`、本文8192文字以内）を許可し、nested details、details外のclosing/sentinel文字列、重複canonical行は拒否します。reactionや任意のbot commentは証跡にしません。finalのno-issues証跡は参照Issueの最終`updated_at`より後でなければなりません。Codexが同一HEADの同一結果をduplicate suppressionした場合だけ、initial/final markerが同じHEAD・本文digestで、unresolved threadが0、かつIssue更新後かつfinal marker前に記録されたcanonical commentをinitial-to-final evidenceとして再利用できます。通常のformal reviewまたは指摘対応経路では、final marker後に別のreview完了証跡を取得します。

## 完了条件

- [ ] `release/vX.Y.Z` の PR が作成されている
- [ ] Draft PR が確認され、初回 `@codex review` が投稿されている
- [ ] 指摘ごとに subagent で修正し、thread reply / resolve が完了している
- [ ] 最新 HEAD・本文 digest への `@codex review` を含め、最低2回 review 済みである
- [ ] 未 resolve thread が 0 件である
- [ ] `Test and Build (...)` と `preflight` が通っている
- [ ] OpenSpec の tasks / DoD と `release-target-check` が通っている
- [ ] `just pr-ready-check "<number>"`（Issue OPEN / 依存更新証跡 / PR range Issue contract / current `pr_body_sha256` を含む）が通った後に `gh pr ready` を実行している
- [ ] Ready 化前に全 review thread を確認し、未 resolve thread が 0 件である
- [ ] Ready 化後にユーザーの merge 承認を得て、global bootstrap skillの `merge --apply` 直前の `just pr-ready-check "<number>"` が成功している
- [ ] release-check / pre-pr の前に対象 version 以前の完了済み OpenSpec change を archive している
- [ ] merge 後に Release workflow が起動している
