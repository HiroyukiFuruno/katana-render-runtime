# Issue起点の変更契約

## 適用範囲

`master` 以外のbranchにある各commitは、このrepositoryのOPEN Issueをcommit messageから参照する。
短縮形は `Refs #64`、完全形は `Refs https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64` とする。

`pre-push` は次の順序を固定する。

1. repository固有の完全検査 `just check`
2. `scripts/hooks/verify_push_issue.py` によるIssue契約検査

Issueが存在しない、CLOSED、別repository、またはbranch固有commitの一部にIssue参照がない場合はpushを拒否する。

## 依存更新証跡

依存manifestまたはlockfileを変更する場合は、参照Issueへ両方の対象pathと次の節を記載する。
推移依存だけを更新してlockfileだけが変わる場合は、依存解決の起点となるmanifestをIssueへ記載すればよい。
API移行が不要な場合も、省略せず理由を記載する。

```markdown
## 依存更新証跡

- 上流公開版: `package-name 1.2.3` と公開URL
- API移行: 必要な変更、または移行不要の理由
- 依存manifest: `Cargo.toml` など変更した全path
- lockfile: `Cargo.lock` など変更した全path
- 検証証跡: 実行したcommandと成功結果
```

検証器は以下を拒否する。

- 上流公開版、API移行、manifest、lockfile、検証証跡の欠落
- 変更したmanifest / lockfile pathがIssue本文にない状態
- `TODO` / `TBD` のままの証跡

## Release cleanup

公開後cleanupは [リリース手順](release.md) の安全条件に従う。
作業中のworktreeを意図的に保持する場合は `git worktree lock <path>` でlocked状態にし、自動削除を拒否させる。

## Pull Requestレビュー運用

Pull Requestは、Issue契約とレビュー結果を同じ変更履歴として追跡できる状態にしてからReadyへ進める。main agentは実装担当ではなく司令塔として、要件、担当分け、ハーネス、統合判断、最終ゲートを管理する。

### 固定フロー

1. Pull Requestを必ずDraftで作成する。
2. Draftの初回レビュー依頼に、対象HEADを記録したinitial markerと `@codex review` を含める。
3. レビュー結果とreview threadを全件取得し、P0/P1/P2などの優先度と対応要否を分類する。CI greenだけではレビュー完了と判定しない。
4. 分離可能な指摘修正は、ファイルまたは非重複責務単位でsubagentへ並列委譲する。main agentは指摘を直列実装せず、各担当の変更範囲を重複させない。
5. 各指摘を修正したら、担当範囲の検証、全体検証、push、該当threadへのreply、threadのresolveを順に行う。返信後は `pull_request_review_id` の `state` と `submitted_at` を確認し、`PENDING` なら自分の当該修正返信だけであることを確認して review event `COMMENT` として submitする（approvalとは別物）。他人の返信や未知のdraftを無断公開せず、submit後は独立readで公開を確認する。App readで返信が見えないだけで増権せず、既に `COMMENTED` なのに見えない場合も対象・pagination・取得scopeを調査する。P0/P1は必須対応とする。
6. pushでHEADが変わるたび、旧HEADのレビューを有効な最終レビューとみなさない。latest initial markerのHEAD・本文digestもcurrent PRと完全一致させ、古いinitial markerと新しいfinal markerを組み合わせない。最新HEADに対してinitial markerからレビューをやり直し、final markerと `@codex review` を付けて再レビューを依頼し、新しい指摘がなくなるまで4〜5を繰り返す。
7. 最新HEAD、レビュー完了、未resolve thread 0、CI、Issue/DoDを次のゲートで機械確認する。ゲートはまず参照IssueがOPENであること、依存更新証跡が揃っていること、PR rangeのIssue契約が完全一致することを検査する。

```bash
just pr-ready-check 72
```

8. `pr-ready-check` 成功後にだけReady化する。Ready化後のmergeはユーザーの明示承認を得てから行い、mergeの直前に同じ `just pr-ready-check 72` を再実行して現在のReady PRを再検証する。mergeはPR checkout外のglobal bootstrap skillが提供する `merge --apply` だけを使う。UI、通常のGitHub CLI merge、人間/admin bypassは使用しない。

```bash
gh pr ready 72
```

active rulesetは更新操作専用であり、exact Integration Appのbypass actor（`bypass_mode=pull_request`）だけを許可し、人間/admin/UI/通常のGitHub CLIによるprotected default branchの更新・mergeとAppによる直接ref更新を拒否する。required checkとconversationの実施主体はclassic branch protectionに置く。classic branch protectionは`enforce_admins=true`、`required_conversation_resolution=true`、`strict=true`を維持し、required contextsをフェーズごとにApp bindingする。bootstrap中は`KRR / PR governance bootstrap`を専用App ID、finalize後は`KRR / PR governance (trusted check)`を専用App ID、`KRR / PR governance review latch`をGitHub Actions `app_id=15368`に固定する。dynamic barrierはdefense-in-depthであり、cold-stateのmerge authorityではない。

### Governance bootstrapの限定例外

通常PRは前節の `pr-ready-check` 成功前にReady化またはmergeしてはならない。`.github/workflows/**` を追加、変更、rename、削除するgovernance bootstrap PRは、trusted PR range検証が意図どおりworkflow変更をblanket denyするため、PR内の例外や検証緩和では解けない。この場合だけ、PR外の専用GitHub Appが固定した最新HEADを独立に検証し、一時Check Run `KRR / PR governance bootstrap` をcompleted/successとして作成または更新する。branch protectionは、そのCheck Runを当該専用App IDに固定したrequired checkとして設定する。

bootstrap PRのReady化とmergeには、上記一時Check Runの成功に加えて、最新HEADのfinal review完了、未resolve thread 0、既存CI、DoDを全て要求する。PR内のallowlist、自己承認、`verify_push_issue.py` の緩和、PR由来workflowによるbootstrap Check Runの発行は禁止する。merge直後に一時Check Runをrequired checkから除去し、専用App IDに固定した `KRR / PR governance (trusted check)` とGitHub Actions `app_id=15368` に固定した `KRR / PR governance review latch` をrequiredへ切り替える。使い捨てPRで両checkを実機smokeし、改変後のfinal review証跡が旧Check Runを失効させることまで確認して完了とする。

操作は次のCLIに固定し、`prepare --apply`（保存済みhuman `gh auth`、expected default branch確認、App-only ruleset先行作成）→`activate --apply`（preexisting exact rulesetと固定証跡のverify-only）→`merge --apply`（App-only保護merge）→`finalize --apply`（merge後再読）→使い捨てsmoke PRの同じApp-only merge route→`verify`の順序を崩さない。各`--apply`の直前にfresh action-time confirmationを取得し、prepareと非mutating `merge` dry-runだけJWT不要、activate/merge/finalize/verifyのlive operationは各実行直前にfreshな`KRR_GOVERNANCE_APP_JWT`だけを環境から受け取る。script自身が各operationのexact KRR repository向けleast-privilege短期IATをmintし、responseのidentity、`expires_at`、scope、permissionsをstrict検証する。mergeは固定base/head/diff/完全allowlist、最新PR本文digest、review完了、未resolve thread 0、Issue OPEN/契約、CI、trusted check、latch、active rulesetのexact Integration App bypass、Contents write、merge APIのexpected head SHAを再取得・照合する。dry-runはmutation 0を確認し、merge API成功後およびfinalize/verifyでmerge commit、ruleset、classic protectionを再取得して永続状態を確認する。JWT/private key/Installation tokenを引数・出力へ出してはならず、PR checkoutのコードをbootstrap evidenceとして実行せず、rollbackでrulesetを外してはならない。

```bash
SCRIPT=/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py
bootstrap_args=(
  --repository HiroyukiFuruno/katana-render-runtime
  --pr <bootstrap-pr-number>
  --expected-base <40-character-base-sha>
  --expected-head <40-character-head-sha>
  --expected-app-id <governance-app-id>
  --allowed-workflow .github/workflows/pr-governance.yml
  --allowed-workflow .github/workflows/pr-governance-status-writer.yml
  --allowed-workflow .github/workflows/pr-governance-review-events.yml
  --allowed-workflow .github/workflows/release.yml
  --allowed-workflow .github/workflows/release-preflight.yml
  --allowed-workflow .github/workflows/test-and-build.yml
  --expected-diff-sha256 <64-character-diff-sha256>
)
# prepare --apply uses stored human gh auth and no JWT; confirm immediately before mutation.
python3 "$SCRIPT" prepare \
  --repository HiroyukiFuruno/katana-render-runtime --pr <bootstrap-pr-number> \
  --expected-default-branch <default-branch> --expected-app-id <governance-app-id> --apply
unset KRR_GOVERNANCE_APP_JWT
# Inject a fresh JWT immediately before activate --apply.
# The skill script mints a bounded IAT. Never echo, persist, or pass either credential as an argument.
export KRR_GOVERNANCE_APP_JWT="${FRESH_JWT:?inject a fresh App JWT outside the command line}"
# action-time confirmation required immediately before this protected mutation
python3 "$SCRIPT" activate "${bootstrap_args[@]}" --apply
unset KRR_GOVERNANCE_APP_JWT
# Run only after the user has approved and the final gate has passed.
# Inject a different fresh JWT before this --apply.
export KRR_GOVERNANCE_APP_JWT="${FRESH_JWT:?inject a fresh App JWT outside the command line}"
# action-time confirmation required immediately before this protected mutation
python3 "$SCRIPT" merge "${bootstrap_args[@]}" --apply
unset KRR_GOVERNANCE_APP_JWT
# Run only after the bootstrap PR has been merged through the protected gate.
# Inject a different fresh JWT before this --apply.
export KRR_GOVERNANCE_APP_JWT="${FRESH_JWT:?inject a fresh App JWT outside the command line}"
# action-time confirmation required immediately before this protected mutation
python3 "$SCRIPT" finalize "${bootstrap_args[@]}" --apply
unset KRR_GOVERNANCE_APP_JWT
# Run only after the disposable smoke PR has also been merged.
export KRR_GOVERNANCE_APP_JWT="${FRESH_JWT:?inject a fresh App JWT outside the command line}"
python3 "$SCRIPT" verify "${bootstrap_args[@]}" --smoke-pr <smoke-pr-number>
unset KRR_GOVERNANCE_APP_JWT
```

このCLIも通常gateの代替ではない。固定HEAD、Issue OPEN、依存更新証跡、PR range契約、Draft/review/CIを検証し、workflow allowlistの完全一致に失敗したら停止する。PR内のworkflow/branch/Issueを条件にした自己例外、`verify_push_issue.py`の緩和、Actions tokenによるbootstrap status発行は禁止する。

レビュー依頼markerは、対象HEADを曖昧にしないため次の形式にする。

```text
<!-- krr-review phase=initial head=<40文字のHEAD SHA> body-sha256=<現在のPR本文のUTF-8 SHA-256小文字hex> -->
@codex review
```

最終レビューでは `phase=final` と最新HEAD SHA、現在のPR本文をUTF-8でSHA-256化した小文字hexを使う。initial/finalの完了証跡は、対応するmarker投稿後かつ最新HEAD・本文digestに対してbot reviewが提出された場合だけとする。本文を同一HEADで変更したら、本文digestを更新したinitial markerからレビューをやり直し、initial reviewとfinal reviewの両方を新しい本文digestへ束縛する。trusted Check Runの`pr_body_sha256`がmissing、duplicate、stale、またはcurrent digestと不一致になった場合も旧successを失効させ、新しいinitial marker→bot review→final marker→bot reviewを完了するまでReady化・mergeしてはならない。`eyes`等のreactionは受付中を示す補助表示であり、永続的な完了証跡として受理しない。レビュー指摘へのreplyとresolveを省略したままReady化してはならない。

#### no-issues Issue commentのcanonical契約

review botの「no issues」はformal reviewではなく、trusted botがPR Issueへ投稿したcanonical commentだけを完了証跡として扱う。本文は `Codex Review: Didn't find any major issues...` または同じcanonical prefixの短文に続き、`**Reviewed commit:** \`<10〜40桁の小文字SHA prefix>\`` を含み、current HEADがそのprefixで始まることを必須とする。`created_at == updated_at` でないcomment、reactionだけの表示、任意のbot commentは受理しない。phaseのmarker間にcanonical候補は高々1件でなければならない。

任意のdetailsは省略またはcanonicalな1つだけを許可する（summaryは `ℹ️ About Codex in GitHub`、本文は8192文字以内）。details内のnested `<details>`、許可されたclosing tag以外の`</details>`、`Codex Review:`、`**Reviewed commit:**` の追加出現などのsentinel/重複はfail-closedで拒否する。finalのno-issues完了は参照Issueの最終`updated_at`より後でなければならない。Codexが同一HEADの同一結果をduplicate suppressionした場合だけ、initial/final markerのHEAD・本文digestが同一で、unresolved threadが0、かつIssue更新後かつfinal marker前に記録されたcanonical commentをinitial-to-final evidenceとして再利用できる。通常のformal reviewまたは指摘対応経路では、final marker後に別のreview完了証跡を取得する。

### オーケストレーションとハーネス

変更ファイルを先に棚卸しし、利用可能な並列枠を確認する。実装、テスト、調査、文書化が互いに独立している場合は、それぞれを別subagentへ委譲する。利用不可のモデルを選ばず、限定実装はLuna、複雑な設計・統合分析はTerraへ切り替える。main agentは結果を鵜呑みにせず、差分、追加fixture、完全ゲートで再検証する。

レビュー運用を守れているかは、口頭確認ではなく次の仕組みで検査する。

- ローカルの `pr-ready-check` がDraft、marker、最新HEAD、レビュー完了、thread解決、CIを検査する。
- trusted default-branch writerがPRの最新HEADを対象に検査し、`KRR / PR governance (trusted check)` Check Runを1件だけcreate-or-updateする。発行App、HEAD、external_idを固定して検証し、branch protectionではこのnameと発行Appの組み合わせを必須化する。
- trusted Check Runのdetails/queryにある`pr_body_sha256`は、GitHub APIから再取得したcurrent PR bodyを正規化せずstrict UTF-8 SHA-256した64桁小文字hexと**ちょうど1個（exactly one）**だけ一致しなければならない。missing、duplicate、stale、または不一致はfail-closedで拒否し、旧successを有効な証跡として再利用しない。
- writerが再計算中のtrusted Check Run instanceだけは、そのwriter自身の内部入力検証から除外する。ただしbranch-protectionのApp binding、latch、source、CI、Issue、review、threadの各検証は省略しない。
- Draftではtrusted Check Runをcompleted-successにせず、ReadyのPRだけを検査して同一HEADのCheck RunをPATCH更新する。writerはPRブランチのworkflowをcheckoutまたは実行しない。
- Check Run発行にはGitHub Actionsの共通`app_id`を使わず、専用のKRR governance GitHub Appを使う。protected environment `pr-governance` に専用App IDとprivate keyを保持する。dispatcherは`actions:write`とinvalidator用`checks:write`、writerは`checks:write` tokenと分離したread-only App token（actions/checks/contents/issues/pull requests read）を使う。branch protectionのrequired checkも、この専用App IDから発行されたものに固定する。
- `KRR / PR governance review latch` はreview sensorの固有GitHub Actions jobである。sensorはread-onlyの`actions`/`checks`権限だけを持ち、secret、checkout、write権限を持たない。Draftでは即時failure、Ready PRでは同一HEAD上のtrusted Check Runをpollし、source_run_idとsource Actions run証跡を厳密照合したcompleted/successだけを受理する。pending、failure、API error、設定欠落、timeout、曖昧なCheck Runはfail-closedにする。
- branch protectionでは`KRR / PR governance (trusted check)`を専用App IDで、`KRR / PR governance review latch`をGitHub Actions `app_id=15368`で、それぞれ必須化する。後者だけのsuccessではmergeできず、前者だけの旧Check Runも別HEADまたは別sensor runには使えない。
- repository Actionsのdefault `GITHUB_TOKEN` はread-onlyに保つ。sensorのread-only pollingをChecks writeへ拡張してはならない。
- trusted PR range検証は`.github/workflows/**`配下の追加、変更、rename、削除をすべて拒否する。sensor workflowがPR merge refで実行されても、改変には新HEADが必要で、そのHEADには専用App Check Runの成功が存在しない。trusted writerが完全なPR range検証後に発行するCheck Runだけがlatchを解放できる。
- `pr-ready-check` がtrusted default SHAを判定するときは、GraphQLの`defaultBranchRef.target`にあるCommit `oid`を正本として使う。compare対象のpathsは固定したbase...headの範囲を取得し、300件境界を超える、欠落する、または応答が曖昧な場合はfail-closedにする。

bootstrap PRでは、merge前に専用Appをinstallし、一時Check Run `KRR / PR governance bootstrap` をclassic branch protectionのbootstrap phase required contextとして当該App IDに固定する。PR外の専用Appが固定HEADのCheck Runをcompleted-successにしたことを確認してからだけmergeする。merge直後はbootstrap contextを除去し、classic branch protectionのfinalized phase required contextsを`KRR / PR governance (trusted check)`は専用App ID、`KRR / PR governance review latch`はGitHub Actions `app_id=15368`に固定して即時切替する。active rulesetは更新操作専用であり、`enforce_admins=true`、`required_conversation_resolution=true`、`strict=true`を含むclassic protectionを人間/admin/UI/通常CLIで緩和しない。Check Runは同一HEADをPATCH再利用し、1000件のstatus永続上限を作らない。使い捨てPRのsmoke完了まで公開運用を完了扱いにしない。

### Single-arbiter reconciliation（現行）

`pr-governance.yml` のconcurrency外`resolve_event` jobは、PR lifecycle、review sensor、CI/release、関連Issue/comment eventを信頼済みdefault-branch入力として厳密に検証し、event relevanceだけをread-onlyで判定する。通常Issue/commentがlocal canonical closerを持たない場合、forkまたはnon-default-base PRの場合は`reconcile=false`としてsingleton reconcilerへ入らない。`pull_request_target` はcheckoutもPRコードの実行もせず、current PRのbase/head/repository/stateをAPIで再束縛する。

writerは全open PRを完全paginationで一回だけsnapshotし、local/default-base PRだけのIssue claimant indexをO(N)で構築して逐次再検証する。固定件数上限やIssue別workerは存在しない。各PRの失敗後も残りを処理し、run全体では失敗を集約する。CI/releaseのworkflow ID・run ID・number・attempt・status・conclusion、review sensor、canonical closer、PR base/headはsuccess直前に再取得する。CI rollupもsuccess直前に再取得し、古い取得結果をsuccess根拠にしない。同workflow・同headの新世代runを検出した場合は旧runがstatusを書かず、source generationをtargetへ結合して再評価する。

関連eventだけが`reconcile-all-open` singletonへ進む。GitHub Actionsのconcurrencyで待機runが置換される順序をFIFOとは見なさない。reconcilerはlock取得後にcurrentの全open local/default-base PRを完全paginationで再列挙し、全current headを`in_progress`化するため、後発の関連eventは置換されたpending eventの対象も同じall-open reconciliationへcoalesceする。正規active writerをActions-write tokenでcancelし、そのterminal drain完了後にだけall-open invalidationを行い、writerをdispatchして`run-name: source=<dispatcher_run_id>`を持つ新writerの登録を確認するまでlockを保持する。writerはrepository-wide `cancel-in-progress: false`で直列化され、cancel/drainによりinvalidatorとwriter mutationを重ねない。singleton mutatorは初回を含めて8.1秒以上空け、rolling 1時間あたり最大445件とする。all-open invalidatorに件数上限はなく、open PRが多数なら一回のreconcilerが複数hourに跨り得る。expected fingerprintはterminal PATCH直前に再読して後発details URLを拒否し、default branchとprovenanceを再確認して各PRを`process`から`finalize`まで逐次確定する。dispatcherは既存trusted Checkが前dispatcherの厳密なmarker付き未処理`in_progress`の場合だけ`carry_pending=1`を次generationへ保存し、writerはcarry対象を先頭に処理する。current PRのDraft判定はstrict booleanで行い、Draftはcarry対象にしない。writerの処理順はcarry対象のnon-Draft、freshなnon-Draft、Draftとし、恒常Draftが予算超過tailをstarveさせない。terminal pendingまたはDraftのCheck Runはcarry対象にしない。全current PRが当該dispatcherの`carry_pending=0/1`に対応する`in_progress`状態と一致しない場合、writerはfail-closedで停止する。これによりrun番号の欠番に依存せず、予算超過分のtailを次回generationへ引き継いで毎run縮小・収束させる。event writerは100件、scheduleは実コスト（既存Check Runの再評価1件、新規terminal更新2件）で400件まで処理する。予算不足時はterminalを書かず、pendingまたはrequired check missingのfail-closedを維持し、scheduleで収束させる。review latchを一つのsensor runへ二重に結び付けない。外部永続hourly limiterがなく、GitHub Actionsの起動・CASにも残余があるため、Issue mutation後に既存trusted successが一時的に残る起動窓は残る。API失敗やcontent rate超過時も未処理headに旧successが一時残存し得るため、merge直前strict gateで再評価する。これは原子的な保証ではなく、厳密な原子保証には外部GitHub Appまたはmerge authorityが必要である。

Issueのopened/edited/deleted/transferred/pinned/unpinned/closed/reopened/assigned/unassigned/labeled/unlabeled/locked/unlocked/milestoned/demilestoned/typed/untyped/field_added/field_removedと、通常Issueへのcomment created/edited/deletedは同じ再評価起点である。dispatcherはeventを型検証し、open PRを完全paginationする。ページ、番号、state、本文、repository、重複に異常があればfail-closedとする。full URLによるclosing referenceは同一repositoryのURLだけを受理し、別repositoryのURLはlocal Issue claimから除外する。その結果local canonical closerを満たさない当該PRは個別failureにする。PR bodyがnullなら空のcloserとして当該PRを個別failureにし、snapshot全体の処理は継続する。変更Issueをsame-repository closing keywordで参照するlocal/default-base PRがあれば、rate budget内で同期pending化して一回のwriter再評価へ渡す。作成・編集前の本文も対象に含めるため、Issue closureの変更時は旧Issueと新Issueのclaimant双方を再評価する。複数Issue closingや複数claimantはwriterの既存ready契約でfailureとなる。GitHubのcontent/API rate超過やAPI失敗時は未処理headに旧successが一時的に残り得るため、後続scheduleとmerge直前のlocal gateで再評価する。Actionsの起動前・API失敗時も同様であり、merge直前のlocal gateが必須である。厳密な原子保証には外部GitHub Appまたはmerge authorityが必要である。

marker投稿後に最新HEADへbot reviewが提出されたことを確認してから、Draftのまま`pr-ready-check`を通してReady化する。Ready化後のreview/Issue/CI変化はdispatcherが到達すればpending化され、最新の全open reconciliationで収束する。起動窓は残るため、merge直前に`just pr-ready-check <PR番号>`を必ず再実行する。Issue契約を復元した場合も必要な最終review、`pr-ready-check`、Ready化を順にやり直す。sourceなしのIssue再評価はreview latchを直接解放しない。

GitHub Actionsではreview threadのresolve/unresolve変更を検知できないため、branch protectionの`required_conversation_resolution=true`を必須にする。新しいreviewまたは未解決threadが同一HEADに追加されても、trusted Check Runの再評価とGitHub native conversation gateの両方でmergeを拒否する。

bootstrap後は使い捨てPRで次を実機確認する。最新HEADに対する専用App Check RunとPR merge SHAに付くActions latchの両方がrequired checkとして評価されること、両方success後にfinal marker commentを編集すると旧bot review証跡が失効してwriterがfailure Check Runへ更新しmergeが拒否されること、そして新しいmarker後のbot review提出とReady化でだけ再びsuccessになることを確認する。reactionは受付中等の補助表示に限り、smokeの成功・回復判定には使用しない。
