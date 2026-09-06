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

`release/vX.Y.Z` から `master` へ Draft PR を作成します。対象 version 以前の完了済み OpenSpec change が archive 済みであることを確認してから、PR 前の gate と Draft PR 作成へ進みます。

```bash
lefthook run pre-pr
pr_url="$(gh pr create --draft --base master --head release/vX.Y.Z --title "Prepare vX.Y.Z release" --body-file <pr-body-file>)"
gh pr view "${pr_url}" --json isDraft --jq '.isDraft'
pr_number="$(gh pr view "${pr_url}" --json number --jq '.number')"
pr_json="$(gh api "repos/<owner>/<repo>/pulls/${pr_number}")"
head_sha="$(jq -r '.head.sha' <<<"$pr_json" | tr '[:upper:]' '[:lower:]')"
body_sha256="$(printf '%s' "$pr_json" | python3 -c 'import hashlib, json, sys
payload = json.load(sys.stdin)
body = payload.get("body")
if not isinstance(body, str) or "\x00" in body or any(0xD800 <= ord(char) <= 0xDFFF for char in body):
    raise SystemExit("PR body must be a valid string without NUL or surrogate characters")
print(hashlib.sha256(body.encode("utf-8", "strict")).hexdigest())')"
review_body="<!-- krr-review phase=initial head=${head_sha} body-sha256=${body_sha256} -->"$'\n@codex review'
gh pr comment "${pr_url}" --body "${review_body}"
```

Draft が `true` であることを確認してから初回 review を依頼します。cloud review を正とし、全 review thread を取得して指摘を分類します。各指摘は責務単位で subagent に委譲し、修正・検証・push 後に該当 thread へ reply して resolve します。

Codex の no-issues 応答は formal review ではなく trusted bot の Issue comment とする。canonical body 全体一致、`Reviewed commit` の10〜40桁hex prefixとcurrent HEADの一致、未編集（`created_at == updated_at`）、各phase windowの候補一意（重複はfail-closed）を確認する。optional details footerはlive canonical summary/structure一致、nested/sentinel拒否、8192文字以下の場合だけ許可する。同一current HEAD・同一PR body digest・unresolved thread 0・Issue freshness（Issue更新後）が揃う場合に限り、initial marker後かつfinal marker前のno-issues証跡をfinalに再利用する。reactionは証跡にせず、formal review/指摘経路はfinal marker後の別evidenceを必須とする。strict marker、App-only merge、initial→final順序を維持する。

レビュー証跡の有効性は current HEAD と current PR body digest に固定する。push 後は旧 HEAD の marker・bot review・trusted success を無効化し、GitHub API で current HEAD/body を再取得して新しい initial marker と cloud review を取得してから final review へ進む。PR本文を編集した場合も、同じ HEAD であって旧 body digest の証跡を無効化し、同じ手順をやり直す。push と本文編集のどちらも無い場合に限り、上記の同一HEAD・同一body digest・Issue freshness・未resolve 0 の条件を満たす既存証跡を再利用できる。

## Phase 5: PR gate

current HEAD・本文 digest に紐づく initial marker と cloud review が成立していることを前提に、初回指摘への対応後、または指摘が無い場合でも、merge 前に同じ current HEAD・本文へ最終 review を依頼します。

```bash
pr_json="$(gh api "repos/<owner>/<repo>/pulls/${pr_number}")"
head_sha="$(jq -r '.head.sha' <<<"$pr_json" | tr '[:upper:]' '[:lower:]')"
body_sha256="$(printf '%s' "$pr_json" | python3 -c 'import hashlib, json, sys
payload = json.load(sys.stdin)
body = payload.get("body")
if not isinstance(body, str) or "\x00" in body or any(0xD800 <= ord(char) <= 0xDFFF for char in body):
    raise SystemExit("PR body must be a valid string without NUL or surrogate characters")
print(hashlib.sha256(body.encode("utf-8", "strict")).hexdigest())')"
review_body="<!-- krr-review phase=final head=${head_sha} body-sha256=${body_sha256} -->"$'\n@codex review'
gh pr comment "${pr_url}" --body "${review_body}"
```

別の push 後は旧HEADのreviewを無効とし、current PRのhead/bodyを再取得してbody digest付きmarkerで再レビューします。PR bodyを編集した場合は同じHEADでも旧markerと旧reviewを無効とし、initial marker→bot review→final marker→bot reviewをやり直します。最低2回のreview、最新HEADのbot review完了、全threadのreply/resolve、未resolve 0を満たすまでReadyに進みません。

次を確認します。

- `Test and Build (macos-latest)` / `ubuntu-latest` / `windows-latest`
- `preflight`
- `just VERSION=vX.Y.Z release-target-check`
- OpenSpec の tasks / DoD
- 最新 cloud review の未対応指摘 0、未 resolve thread 0

```bash
gh pr checks --watch "${pr_url}"
just VERSION=vX.Y.Z release-target-check
```

CI green だけでは Ready 条件を満たしません。指摘が出た場合は修正→通常の commit/push→reply/resolve後、pushまたは本文変更で旧証跡を無効化し、GitHub APIでcurrent HEAD・本文を再取得してinitial marker→cloud review→final marker→cloud reviewを再実施します。push・本文変更がない場合は、既存のcurrent initial証跡を維持したままfinal reviewを実施します。

## Phase 6: Ready 化と merge 承認

全 gate とレビューを確認した後、Draft のまま専用ゲートを実行し、成功後だけ Ready 化します。

```bash
just pr-ready-check "<number>" && gh pr ready "${pr_url}"
```

`pr-ready-check` は参照Issueが OPEN であること、依存更新証跡が揃っていること、PR range の Issue contract が完全一致すること（不足・余分を含む）を先に検証します。Ready 化前とglobal bootstrap skillの `merge --apply` 直前の両方で、review markerのHEAD/body digestとtrusted Check Run evidenceのHEAD/external_idを同一境界に一致させる。trusted evidence の query にある `pr_body_sha256` は **ちょうど1個** の64桁小文字hexで、GitHub APIから再取得した current PR本文の strict UTF-8 SHA-256 と完全一致しなければならない。missing、duplicate、old digest、または異なるdigestは fail-closed である。Ready 化前に merge 承認を求めず、Ready 化後にユーザーの merge 承認を得ます。承認後、同じ `just pr-ready-check "<number>"` をmergeの直前に再実行し、Ready PRの最新Issue/marker/thread/CI/base/headとこの一意なtrusted digest bindingを再検証してから、PR checkout外のApp-only `merge --apply`を実行します。承認前に merge してはいけません。

### governance workflow の初回 bootstrap

PR が `.github/workflows/` の governance workflow 自体を追加・変更する初回 bootstrap に限り、専用skill scriptをPR checkout外で次の順序に固定します。通常PRへ例外を拡張せず、PR内のworkflow、branch名、Issue、Check Runを自己承認の根拠にしません。

1. `prepare --apply`: 保存済みの人間用 `gh auth` でexpected default branchを確認し、App-only active rulesetを先行作成する。fresh JWTは不要です。
2. `activate --apply`: fresh JWTを環境変数に設定し、scriptがexact repository・operation-specific least-privilegeでmintした短期IATのresponse identity/expiry/scope/permissionsをstrict検証したうえで、一時Check Runと固定base/head/diff/allowlistをverify-only確認する。
3. Ready化後、ユーザーのfresh merge承認を取得し、同じ`pr-ready-check`を直前に再実行してから`merge --apply`を実行する。mergeはactive rulesetのexact Integration App bypassとclassic branch protectionのrequired conversation resolution、phase-exact trusted/latch、最新body digest/review/thread/Issue/CIを再検証し、Contents writeを含む短期IATとmerge API expected SHAを検証してから行う。
4. `finalize --apply`: merge後にmerge commit、ruleset、classic protectionを再取得し、永続required contextsを確認する。
5. 同じApp-only merge routeで使い捨てsmoke PRをmergeし、別のfresh JWTからread-only IATをmintして`verify`でsmoke結果を確認する。

active rulesetは更新操作専用であり、exact Integration Appのbypass actor（`bypass_mode=pull_request`）だけを許可し、人間/admin/UI/通常のGitHub CLIによるprotected default branchの更新・mergeとAppによる直接ref更新を拒否します。required checkとconversationの実施主体はclassic branch protectionに置きます。classic branch protectionは`enforce_admins=true`、`required_conversation_resolution=true`、`strict=true`を維持し、required contextsをフェーズごとにApp bindingします。bootstrap中は`KRR / PR governance bootstrap`を専用App ID、finalize後は`KRR / PR governance (trusted check)`を専用App ID、`KRR / PR governance review latch`をGitHub Actions App ID `15368`に固定します。dry-runはmutation 0を証明します。人間/admin/UI/通常のGitHub CLI merge（`gh pr merge`を含む）は禁止し、dynamic barrierはdefense-in-depthであってcold-stateのmerge authorityではありません。各`--apply`の直前にfresh confirmationを取得し、prepare以外の各コマンドではfresh JWTのみを環境へ設定します。JWT/private key/IATをargv・出力へ出さず、rollbackでもrulesetを外しません。

## Phase 7: merge と自動リリース

承認後は、直前の `just pr-ready-check "<number>"` が成功した場合だけ、PR checkout外のglobal bootstrap skillによるApp-only `merge --apply`を実行し、release 前に archive 済みの OpenSpec change と Release workflow の結果を確認します。merge APIにはcurrent expected head SHAを渡し、merge commitとactive rulesetを再取得します。

```bash
SCRIPT=/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py
python3 "$SCRIPT" merge \
  --repository <owner>/<repo> --pr "<number>" \
  --expected-base <base-sha> --expected-head <head-sha> \
  --expected-app-id <governance-app-id> \
  --allowed-workflow <every-changed-workflow-path> \
  --expected-diff-sha256 <binary-full-index-diff-digest> --apply
gh run list --workflow Release --limit 5
```

## 完了条件

- [ ] Draft PR 作成と初回 marker 付き review
- [ ] 指摘の修正、検証、thread reply / resolve
- [ ] 最新 HEAD/body digest の final marker bot review と未 resolve 0
- [ ] CI / DoD / `release-target-check` / `pr-ready-check` PASS
- [ ] `just pr-ready-check "<number>"`（Issue OPEN / 依存更新証跡 / PR range Issue contract / current `pr_body_sha256` exactly one を含む）後に Ready 化
- [ ] Ready 化後に merge 承認を得て、global bootstrap skillの `merge --apply` 直前の `just pr-ready-check "<number>"` 成功後にApp-only merge
- [ ] release-check / pre-pr の前に対象 version 以前の完了済み OpenSpec change を archive し、Release workflow を確認
