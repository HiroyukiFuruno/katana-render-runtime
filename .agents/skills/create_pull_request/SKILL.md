---
name: create_pull_request
description: katana-diagram-renderer の Pull Request を、自己レビューと品質ゲート後に GitHub CLI で作る。base branch を文脈から確認し、PR 本文に検証結果を含める。
---

# Create Pull Request

PR 作成前に、差分、検証、base branch を確認します。PR は必ず Draft で作成し、cloud review と指摘対応を完了してから Ready にします。
推測で `master` や `main` を選びません。

governance bootstrap と保護mergeは、PR外の絶対path `/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py` の `prepare` / `activate` / `merge` / `finalize` / `verify` に限定する。`prepare --apply` は保存済みの人間用 `gh auth` でexpected default branchを確認してApp-only rulesetを先行作成するためJWT不要、非mutating `merge` dry-runも保存済み認証によるpublic readだけを使う。`activate --apply` はそのrulesetと固定証跡のverify-only、`merge --apply` はApp-only merge、`finalize --apply` はmerge後の再読、`verify` はsmoke確認とする。activate/merge/finalize/verifyの各実行直前にfreshな `KRR_GOVERNANCE_APP_JWT` だけを環境へ設定し、script自身がexact KRR repository・operation-specific least-privilegeで短期IATをmintする。IAT responseのidentity・expiry・scope・permissionsをstrict検証し、JWT/private key/IATをCLI引数・出力へ出さない。`--expected-base --expected-head --expected-app-id --expected-diff-sha256`、expected default branch、完全な `--allowed-workflow`、current body digest/review/thread/Issue/CI/trusted/latch、ruleset exact App bindingを固定し、merge APIにはexpected head SHAを渡して成功後にmerge commit/rulesetを再読する。人間/admin/UI/通常のGitHub CLI merge（`gh pr merge`を含む）は禁止し、PR checkoutのコードをevidenceとして実行しない。通常の `pr-ready-check` は緩和しない。

レビュー証跡の no-issues 契約: Codex の no-issues 応答は formal review ではなく trusted bot の Issue comment として扱う。canonical body 全体が一致し、`Reviewed commit` の 10〜40 桁 hex prefix が current HEAD に一致する未編集 comment（`created_at == updated_at`）だけを受理する。initial/final 各 phase window の候補は高々1件（重複は fail-closed）。optional details footer は live canonical summary/structure 一致、nested/sentinel 拒否、8192文字以下の場合だけ許可する。同一 current HEAD・同一 PR body digest・unresolved thread 0・Issue freshness（Issue 更新後）の全条件を満たす場合に限り、initial marker 後かつ final marker 前の no-issues 証跡を final に再利用できる。reaction は証跡にしない。formal review/指摘対応経路は final marker 後の別 bot review/thread evidence を必須とする。strict marker、App-only merge、initial→final 順序は維持する。

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
pr_url="$(gh pr create --draft --base "<base-branch>" --head "<current-branch>" --title "<title>" --body-file "<body-file>")"
gh pr view "$pr_url" --json isDraft --jq '.isDraft'
pr_number="$(gh pr view "$pr_url" --json number --jq '.number')"
pr_json="$(gh api "repos/<owner>/<repo>/pulls/$pr_number")"
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
gh pr comment "$pr_url" --body "<!-- krr-review phase=initial head=$head_sha body-sha256=$body_sha256 -->"$'\n@codex review'
```

`--base` は必須です。`isDraft=true` を確認してから初回 review を依頼します。

## 5. Review と指摘対応

```bash
pr_json="$(gh api "repos/<owner>/<repo>/pulls/$pr_number")"
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
gh pr comment "$pr_url" --body "<!-- krr-review phase=final head=$head_sha body-sha256=$body_sha256 -->"$'\n@codex review'
```

初回・最終 review とも結果を取得し、review thread は全ページ確認します。指摘は責務単位で subagent に委譲し、修正→検証→通常の commit/push→該当 thread への reply→resolve の順で処理します。markerはcurrent PRのhead/bodyを依頼直前に再取得して生成し、bodyがstringでない、NULまたはsurrogateを含む、UTF-8 strictで符号化できない場合はfail-closedで停止します。bot reviewが最新HEADに提出された場合だけ完了とします。指摘対応で push または PR body 編集が発生した時点で、旧 initial/final marker と旧 review 証跡をすべて無効化し、current HEAD/body digestで新しい initial marker→cloud reviewを完了してから、同じ current HEAD/body digestで final marker→cloud reviewへ進みます。pushやbody変更後に旧initialと新finalだけを組み合わせる経路は禁止です。HEAD/bodyが不変でpushもbody編集もない場合だけ、既存のinitialからfinalへ進めます。未対応指摘と未resolve threadが0件になるまで繰り返します。

## 6. Ready 化と承認後 merge

CI、self-review、lint、test、coverage、OpenSpec/DoD、最新 HEAD review、未resolve 0 を確認した後だけ、次を実行します。各 Issue の `non-Draft target` は 256 件以下である（256 non-Draft target invariant）ことも確認します。超過した場合は bypass せず、影響する PR を Draft に戻すか closing reference を外してから merge 前に解消します。
- `pr-ready-check` は最初に、参照IssueがOPENであること、依存更新証跡が揃っていること、PR rangeのIssue契約が完全一致することを検査します。trusted Check Run evidenceの`pr_body_sha256`がcurrent PR body digestとexactly one一致し、missing、duplicate、stale digestをfail-closedで拒否することも確認します。

### Governance workflow の初回導入・改修

対象PRが governance workflow 自体の初回導入または改修である場合だけ、PR checkout外の専用skill scriptを次の順序で実行します。通常PRへ例外を拡張せず、PR内の条件分岐・ブランチ名・Issue番号・PR author・workflow変更を自己承認の根拠にしません。

1. `prepare --apply`: 保存済みの人間用 `gh auth` でexpected default branchを確認し、App-only active rulesetを先行作成する。fresh JWTは不要です。
2. `activate --apply`: fresh JWTだけを環境変数へ設定し、scriptがexact repository・operation-specific least-privilegeでmintした短期IATのresponse identity/expiry/scope/permissionsをstrict検証して、固定base/head/diff/allowlistと一時Check Runをverify-only確認する。
3. `pr-ready-check`成功後にReady化し、ユーザーのfresh merge承認を取得する。merge直前に同じ`pr-ready-check`を再実行し、global skillの`merge --apply`でactive rulesetのexact Integration App bypassとclassic branch protectionのrequired conversation resolution、phase-exact trusted/latch、最新body digest/review/thread/Issue/CI、Contents write IAT、merge API expected SHAを再検証して保護mergeする。
4. `finalize --apply`: merge後にmerge commit、ruleset、classic protectionを再取得し、永続required contextsを確認する。
5. 同じApp-only merge routeで使い捨てsmoke PRをmergeし、別のfresh JWTからread-only IATをmintして`verify`でsmoke結果を確認する。

active rulesetは更新操作専用であり、exact Integration Appのbypass actor（`bypass_mode=pull_request`）だけを許可し、人間/admin/UI/通常のGitHub CLIによるprotected default branchの更新・mergeとAppによる直接ref更新を拒否します。required checkとconversationの実施主体はclassic branch protectionに置きます。classic branch protectionは`enforce_admins=true`、`required_conversation_resolution=true`、`strict=true`を維持し、required contextsをフェーズごとにApp bindingします。bootstrap中は`KRR / PR governance bootstrap`を専用App ID、finalize後は`KRR / PR governance (trusted check)`を専用App ID、`KRR / PR governance review latch`をGitHub Actions App ID `15368`に固定します。dry-runはmutation 0を証明します。人間/admin/UI/通常のGitHub CLI merge（`gh pr merge`を含む）は禁止し、dynamic barrierはdefense-in-depthであってcold-stateのmerge authorityではありません。各`--apply`直前にfresh confirmationを取得し、prepare以外はfresh JWTのみを受け取ります。JWT/private key/IATをargv・出力へ出さず、rollbackでもrulesetを外しません。

```bash
just pr-ready-check "<pr-number>" && \
  gh pr ready "<pr-number>"
```

Ready 化後に fresh merge 承認を依頼し、承認後はglobal skillの`merge --apply`直前に同じ `just pr-ready-check "<pr-number>"` を再実行してReady PRの最新Issue/marker/thread/CI/base/head/body digestとtrusted/latchを再検証した場合だけApp-only mergeを実行します。承認前のmerge、人間/admin/UI bypass、`--admin`、`--no-verify`、DraftなしPR作成は禁止です。

```bash
SCRIPT=/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py
python3 "$SCRIPT" merge \
  --repository <owner>/<repo> --pr "<pr-number>" \
  --expected-base <base-sha> --expected-head <head-sha> \
  --expected-app-id <governance-app-id> \
  --allowed-workflow <every-changed-workflow-path> \
  --expected-diff-sha256 <binary-full-index-diff-digest> --apply
```

CI が失敗した場合は修正して同じ gate に戻ります。

## 報告

- PR URL
- base/head
- 検証結果
- CI 状態
