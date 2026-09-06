# katana-diagram-renderer Agent Rules

## Branch Policy

- 公開配布（crates.io）、release tag、公開CLI、公開API、package metadata に影響しない変更は `master` 直接作業でよい。
- 公開配布や release に影響する変更は、作業前に branch 方針を確認する。
- ユーザーが push を明示した場合は、ローカル commit で止めず、通常の `git push` まで実行する。
- pre-push が失敗した場合は回避せず、失敗した検査を修正してから再度 push する。

## Release Inclusion Gate

- ユーザーが特定の修正を指定versionへ抱き合わせるよう指示した場合、release対象commitを `scripts/release/verify-release-target.py` の `REQUIRED_RELEASE_COMMITS` に固定する。
- `release-target-check`、PR作成、mergeの各時点で、release branchのHEADが全必須commitを含むことを `git merge-base --is-ancestor` で機械検証する。
- 別release branchのversion bump、tag、GitHub Release、crates.io公開が成功していても、必須commitを含まない場合は指定releaseの完了として扱わない。

## PR Review Gate

- Pull Request は必ず Draft として作成する。Ready PR を直接作成してはならない。
- Draft 上で初回 cloud review を依頼し、review thread を全件取得・分類する。review依頼コメントは `<!-- krr-review phase=initial head=[0-9a-f]{40} body-sha256=[0-9a-f]{64} -->` の厳密な属性順序・空白・小文字hex文法にする。投稿直前にcurrent PR本文を再取得し、string以外、NUL、lone surrogate、UTF-8 strict不能をfail-closedで拒否して、正規化しないUTF-8 bytesのSHA-256を記録する。Cloud review は GitHub の approving review ではないため、approval として数えない。
- review botの「no issues」はformal reviewではなく、trusted botがPR Issueへ投稿したcanonical commentだけを完了証跡として受理する。本文は `Codex Review: Didn't find any major issues...` または同じcanonical prefixの短文に続き、`**Reviewed commit:** \`<10〜40桁の小文字SHA prefix>\`` を含み、current HEADがそのprefixで始まらなければならない。`created_at == updated_at` を必須とし、phaseのmarker間にcanonical候補は高々1件とする。任意のdetailsは省略またはcanonicalな1つだけ（summary `ℹ️ About Codex in GitHub`、本文8192文字以内）を許可し、nested details、details外のclosing/sentinel文字列、重複canonical行は拒否する。reactionや任意のbot commentは証跡にしない。
- 分離可能な指摘修正は subagent へファイルまたは責務単位で並列委譲し、main agent はハーネス、統合判断、検証を担当する。
- 各指摘は修正、push、thread への reply、resolve まで完了させる。修正後は投稿直前にcurrent PR本文を同じstrict条件で再取得し、`<!-- krr-review phase=final head=[0-9a-f]{40} body-sha256=[0-9a-f]{64} -->` の厳密なmarkerに最新HEADと本文digestを記録して最終 cloud review を依頼する。latest initial markerのHEAD・本文digestもcurrent PRと完全一致しなければならず、pushまたは本文編集後に古いinitial markerと新しいfinal markerを組み合わせてはならない。本文を同一HEADで変更した場合も旧review証跡を無効として、更新済みinitial markerからやり直す。push 後は旧 HEAD の最終 review を無効として扱う。
- finalのno-issues証跡は参照Issueの最終`updated_at`より後でなければならない。Codexが同一HEADの同一結果をduplicate suppressionした場合だけ、initial/final markerが同じHEAD・本文digestで、unresolved threadが0、かつIssue更新後かつfinal marker前に記録されたcanonical commentをinitial-to-final evidenceとして再利用できる。通常のformal reviewまたは指摘対応経路では、final marker後に別のreview完了証跡を取得する。
- CI green だけを review 完了や merge 準備完了の根拠にしない。最新 HEAD の review 完了、未resolve thread 0、CI / DoD PASS を `just pr-ready-check <number>` で機械確認する。この local gate は review検証の前に、参照IssueがOPENであること、依存更新証跡、PR rangeのIssue契約をfail-closedで検証する。trusted Check Run evidence のqueryにある `pr_body_sha256` はちょうど1個だけでcurrent PR本文digestと完全一致しなければならず、missing、duplicate、stale、異なるdigestはfail-closedとする。
- pr-ready-check が成功した後だけ gh pr ready で Ready 化する。直接の Ready 化やUI操作でgateを迂回しても PR governance checkがmergeを拒否する。Ready 化後にユーザーへ merge 承認を求め、承認後かつmergeの直前に同じ `just pr-ready-check <number>` を再実行してReady PRの最新Issue/marker/thread/CI/base/head/body digestとtrusted Check Runの一意な`pr_body_sha256`を再検証する。mergeはPR checkout外のglobal skill `/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/SKILL.md` が定める `merge --apply` 操作だけを使い、UI、通常のGitHub CLI merge、人間/admin bypassは禁止する。merge APIには再検証済みのexpected head SHAを渡し、成功後にmerge commitとrulesetを再取得して確認する。承認前にmergeしない。
- active rulesetは更新操作専用であり、exact Integration Appのbypass actor（`bypass_mode=pull_request`）だけを許可し、人間/admin/UI/通常のGitHub CLIによるprotected default branchの更新・mergeとAppによる直接ref更新を拒否する。required checkとconversationの実施主体はclassic branch protectionに置く。classic branch protectionは`enforce_admins=true`、`required_conversation_resolution=true`、`strict=true`を維持し、required contextsをフェーズごとにApp bindingする。bootstrap中は`KRR / PR governance bootstrap`を専用App ID、finalize後は`KRR / PR governance (trusted check)`を専用App ID、`KRR / PR governance review latch`をGitHub Actions App ID 15368に固定する。dynamic barrierはdefense-in-depthであり、cold-stateのmerge authorityではない。
- `.github/workflows/**` を変更するgovernance bootstrapだけは、local gateのworkflow blanket denyを緩和しない。PR外の専用GitHub Appが固定HEADを独立検証して一時Check Run `KRR / PR governance bootstrap` をcompleted/successにし、classic branch protectionのbootstrap phase required contextとして当該App IDに固定したうえで、最新HEAD review完了、未resolve thread 0、既存CI / DoDを全て満たした場合だけReady / mergeの例外とする。PR内の例外、自己承認、`verify_push_issue.py` の緩和は禁止する。merge直後にbootstrap contextを除去し、classic branch protectionのfinalized phase required contextsとして専用Appの `KRR / PR governance (trusted check)` とActions `app_id=15368` の `KRR / PR governance review latch` をrequiredへ切り替え、使い捨てPRのsmoke完了までをDoDとする。
- bootstrap操作はPR外の専用skill script `/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py` の `prepare` / `activate` / `merge` / `finalize` / `verify` に限定する。`prepare --apply` は保存済みの人間用`gh auth`でexpected default branchを確認し、App-only active rulesetを先行作成するためJWT不要、非mutating `merge` dry-runも保存済み認証によるpublic readだけを使う。`activate --apply`はpreexisting exact rulesetと固定証跡のverify-onlyとする。各`--apply`の直前にfresh confirmationを取得し、activate/merge/finalize/verifyは各実行直前にfreshな`KRR_GOVERNANCE_APP_JWT`を環境変数から受け、script自身がoperation-specific least-privilegeでexact KRR repository向け短期IATをmintする。IAT responseのidentity、expires_at、scope、permissionsをstrict検証し、`merge`は最新body digest、review、thread、Issue、CI、trusted、latch、固定base/head/diff/allowlist、ruleset exact App binding、Contents write、merge API expected SHAを再検証して保護mergeする。dry-runはmutation 0を確認し、finalize/verifyとmerge後はrulesetとclassic protectionを再読する。JWT/private key/IATをCLI引数・出力へ出さず、PR checkoutのコードを実行せず、rollbackでもrulesetを外さない。人間/admin/UI/通常のGitHub CLI merge（`gh pr merge`を含む）は禁止する。

## Orchestration Gate

- main agent は司令塔として、設計、ハーネス、担当分離、統合レビュー、最終ゲートを担う。分離可能なreview指摘修正をmainが直列実装しない。
- 変更ファイルと責務を先に棚卸しし、同時実行枠の範囲で1ファイルまたは非重複責務ごとにsubagentへ並列委譲する。空き枠を放置して直列化しない。
- subagent起動前に、最新のユーザー指示と利用可能モデルを確認する。利用不可と明示されたモデルを選ばず、限定実装はLuna、複雑な設計・統合分析はTerraへ切り替える。
- main agent はsubagent結果を鵜呑みにせず、追加fixture、差分レビュー、完全ゲートで統合判定する。
