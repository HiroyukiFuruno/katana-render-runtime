# v0.4.20 作業台帳

DoD: #73/#74/#75/#76を含むv0.4.20を公開し、対応Issueと不要な作業資源を整理する。依存更新後もcoverageと品質スコア99以上を含む既存完全ゲートを維持する。

## ユーザーフィードバック

- [/] Rust/JS最新版を照合。crc32fast 1.5.1→1.5.2、JS既に最新。TS6強制downgrade除去。更新後の全検査結果PASS。
  - 2026-09-13 追加指示で再照合。cargo upgrade dry-runは34 packages最新、cargo outdated --workspaceは直接・推移依存とも更新候補なし。bun update --latestは128 packages/no changes、bun outdatedも候補なし。runtime updater全6種already latest、runtime-asset-script-test 33件PASS。今回追加の依存差分はなく、既存の未commit更新を保全。
- [/] Issueコメントだけでレビュー未応答と判断した誤りを訂正。formal reviewにP1/P2到着を確認し、並列修正に移行。
- [/] 古いskillを是正・検証（main、Draft必須・全review観測・保護merge・公開/cleanup DoD）。global取得helper11tests、global bootstrap既存phase矛盾修正107tests。

## 残作業

- [x] P1 3996222743: terminal release treeとsquashの一致検証。a65a89aまでpush、reply/resolve済み。
- [x] P2 3997311759: retained DOM descendantsをDropが消さない。dc481cfをpush、reply/resolve済み。
- [x] P2 3997311761: TypeScript 6強制downgradeを除去。23acb8fをpush、reply/resolve済み。
- [x] 最終差分の統合レビュー、完全release-check、描画スコア、依存最新性を確認
  - 未到達2行にsole-owner template Drop回帰を追加。再実行でcoverage 22,495行・未到達0・100% PASS、Mermaid full/CI PASS（代表最小99.59）。Draw.io完全比較も完走（後段に結果）。全更新raw log: /var/folders/ql/4640yx8s22zg367pjjld7yc00000gn/T/krr-depends-update-all.Dyy9IKJC6U。
  - release-openspec-archive / release-verify PASS。配布crate357files、8,512,055/10,485,760 bytes、配布crate自体1004tests PASS（ignored1）、publish dry-run成功。公開そのものは未実行。
  - svgimageの実日付だけ変わった参照画像4パスはHEADへ復元。既存Date契約は変更せず、Issue #80へ再現性改善を分離。元referenceへの99ゲート再比較も完了（後段に結果）。
  - 一括更新本体は全工程完走。Draw.io full25区分、CI20pairs/min99.50、Mermaid CI/min99.59。元svgimage referenceで6pairs/min99.70。全29reportはpassed=true。終了コード記録wrapperのみzsh予約変数statusへの代入でexit1となったため、本体exit0を捕捉したとは記録せず、全工程ログとreportの成功を根拠とする。今後はtask_exit_code等の非予約名を使用する。
- [ ] push、各指摘reply/resolve、最新HEAD/bodyのinitial/final review、readiness gate
- [ ] 保護merge、Release workflow成功、GitHub Release・crates.io公開確認
- [ ] Issue更新、不要branch/worktree/stashを安全に整理

禁止: 品質ゲート緩和、通常CLI/admin merge、他人の差分破棄、未検証の完了報告。

## 新HEADのレビューと公開経路

- [x] a65a89aまで4commitを通常push。既知3threadへ公開reply/resolve。最新HEADのinitial marker 5651431554を投稿。
- [x] 最新HEADの4OS CI・preflight成功。
- [ ] 追加P2 PRRT_kwDOSdqU6c6h2yTG: observer refresh timeoutでruntimeをdiscardする（observer_review_repairs）。
- [ ] 追加P2 PRRT_kwDOSdqU6c6h2yTJ: edge-adjacent intersectionを正しく扱う（observer_review_repairs）。
- [ ] 新修正後のbundle生成、完全品質ゲート、最終non-gate commitへterminal pin更新、再push/review。
  - 追加2件の修正とfocused 8件・lint・AST・fmtはPASS。完全release-checkは通常検査PASS後、coverage 22,570行中未到達1行で停止（/tmp/krr-release-check-final.fIJyUd）。ゲートは緩和せずテスト補完中。描画比較を並行実行。
  - テストhelperの未到達error closureへ回帰を追加し、focused 9件・AST・fmt PASS。製品コードはcoverage都合で変更していない。完全release-checkを /tmp/krr-release-check-final-coverage-retry.log へ記録して再実行。
  - 再実行は終了コード0で全工程PASS。coverage 22,576行/未到達0・2,855関数/未到達0、配布crateテストとpublish dry-runも成功。公開自体は未実施。
  - Rust/npm/Maven/GitHubの最新公開版をread-only再照合し、追加更新なし。Mermaid full/CI PASS（CI最小99.59）、Draw.io full/CIは実行中。
- [/] ユーザーが通常release用の専用App admission追加を承認。global script実装と契約テストを別担当で並行実施。PR #72は前提化せず、既存CI/review/App保護を維持する。

Issue #75は「#72統合を公開の前提にしない」と明示する。現行global scriptのbootstrap専用制約は通常release modeを追加して解消する。専用Appが固定SHA/body/diffとIssue/review/CIを独立検証し、専用Check Runを既存checksを保持したままrequiredへ追加する。対応表・保護phase・再検証契約はglobal skillのreferences/release-admission-contract.mdへ固定。保護緩和や通常gh mergeは行わない。

## 0d601be の再レビュー

- [x] f9ed707/5f178db/0d601beを通常push。追加observer2件は公開reply/resolve済み。
- [x] 4OS CIとpreflightすべてPASS。Mermaid/Draw.io full・CIは終了コード0（/tmp/krr-visual-final-20260913-retry.log）。
- [ ] 3999114957 / PRRT_kwDOSdqU6c6h3bir: mixed serif/sansのCJK候補欠落（mixed_cjk_repair）。
- [ ] 3999114958 / PRRT_kwDOSdqU6c6h3bis: fontconfigなしの名前付きfont探索（native_font_lookup_repair）。
- [ ] 3999114960 / PRRT_kwDOSdqU6c6h3bit: observer reflow後のscroll clamp（observer_scroll_reclamp）。
- [ ] 各修正の固有再現テスト、完全ゲート、最終terminal更新、push/reply/resolve、新HEAD initial/final review。

フォント2件は過去の汎用「対応済み」返信と現コードが一致しなかった。過去の返信や全体CIだけを修正証拠にせず、指摘ごとの現HEAD・再現入力・assert・検証結果を照合してからresolveする。Issue #78へ延期済みの別項目は今回へ混ぜない。

追加3件の個別検証:
- mixed CJK: 修正前RED・修正後GREEN。既存sans/serif定数を合成し、混在時の両集合包含とserif単独の非混入を確認。focused 44件、AST PASS。
- native font: macOS Helveticaをfontconfigなしで探索。File/Binary同familyの実fontでFileのみ選択を検証。system module 10件・strict lint PASS。Windows Segoe UI専用回帰を追加し、実行はWindows CIで確認する（ローカルcross実行はSDK不足で未検証）。
- observer scroll: 修正前handler offset 400でRED、修正後GREEN。handler観測値・session.scroll_y・新max_scrollの完全一致をassert。observer 9件・strict lint PASS。
- 統合release-check: /tmp/krr-release-check-font-scroll-final.log、描画全比較: /tmp/krr-visual-font-scroll-final.log を実行中。個別PASSだけで公開完了やreview解決とは扱わない。
- 統合通常検査・coverageは22,580行/未到達0でPASS。配布crate並列テストでcold RSS増分66,704KiBにより失敗。配布crateの同一テストを完全修飾名で単独実行すると1件PASS（増分10,944KiB、owned556,216B）。プロセス全体RSSに他testの割当てが混入する測定上の問題として、同一test executableの専用子プロセスへcold測定を隔離。64MiB/2MiB/64facesの全基準は不変、子testが実際に1件PASSしたことも確認する。
- RSS隔離後のfocused test PASS。完全再検証を /tmp/krr-release-check-isolated-rss-final.log で実行中。
- RSS隔離の追加で親testfileがASTの300行上限を超えたため、メモリ検証のtest/helperを専用moduleへ分離。親238行、新module87行、focused1件・AST・strict lint・fmt・diff-check PASS。完全再検証は /tmp/krr-release-check-memory-module-final.log。
- 再利用手順は [release-review-tips.md](release-review-tips.md) にまとめた。
- 最終再実行は終了コード0で完全PASS。coverage22,580行/2,858関数とも未到達0。360fileの配布crate並列testとpublish dry-runも成功。フォント修正acec216、scroll修正50630ab、RSS隔離d0a65bbへcommit済み。Mermaid full/CIとDraw.io fullは再比較PASS、Draw.io CIは実行中。push後は最新commitのWindows Segoe UI回帰を含むCIとinitial/final reviewを再確認する。

## 044a655 の公開前検証

- [x] Draw.io CIを含む全比較が終了コード0。29 reports/757 entriesすべてPASS、最小99.01819。参照画像・99基準は不変。
- [x] terminalをf1d23c1へ固定し、044a655まで通常push。release-target契約12件PASS。
- [x] CJKとscroll指摘へ固有検証を返信しresolve。initial review marker 5652267246を投稿。
- [x] 044a655のWindows CI（job103699865830）のunit-test成功を確認。Windows指摘への追記APIは502/不正JSON/GraphQL server errorで未投稿を確認しており、threadは未resolveのまま保全。
- [ ] Linux preflight run34748189346: font_system.rsのcoverage未到達1行で失敗。macOSでの完全PASSと区別し、Linux分岐のテストを補完する（linux_font_coverage担当）。閾値は変更しない。
- [ ] 修正を含む最終HEADのCI・initial/final reviewとApp admissionを再検証し、保護merge・公開・Issue整理へ進む。公開操作は未実施。

## 044a655 の追加指摘・CI是正

- [ ] P1 3999248718 / PRRT_kwDOSdqU6c6h3x7m: Draw.io資産45,764,460Bをbasic描画でも全展開・永続保持。既存groupごとに独立圧縮し、selectorで必要groupだけ展開、raw全体OnceLockを廃止する（drawio_selective_archive）。非選択groupの非展開を回帰契約にする。
- [ ] P2 3999248722 / PRRT_kwDOSdqU6c6h3x7p: hidden/detached observer targetのzero fallback rectをedge接触と誤判定。layout参加有無を区別し、実zero-area接触は維持する（mermaid12_sequence_fix）。
- [ ] Linux coverage: native機能がないOSで到達不能なnative優先分岐をコンパイルしないcfg構造へ整理。非native OSは同じfontconfig実装へ直接委譲し、不要stubを撤去する。品質条件は不変。
- [x] Windows検証追記3999249030は公開COMMENTED（review5190263943、08:50:02Z）を確認。resolve APIは引き続きserver error、未解決状態を確認。API回復後に再検証して個別resolveする。

追加修正の統合証跡:
- Linux cfg整理: native OSだけnative探索分岐を持ち、非native OSは従来と同じfontconfigへ直接委譲。個別テスト・strict lint・fmt PASS。Linux実coverageは新HEAD CIで再確認する。
- observer: bridge回帰と実HTML host再layoutの両方でhidden/detachedのtrue→falseを確認、zero-area edge接触も維持。focused・runtime bundle・strict lint・AST・fmt PASS。並行buildへの影響を避けたため修正前REDは未取得。新helperのmissing-node error経路にも回帰を追加。
- Draw.io: basic.xmlを43,925Bの独立groupへ分割し、全raw OnceLockを廃止。main独立比較で旧HEADの466資産/45,764,460Bが新34groupsと全byte一致、欠落・重複なし。Bun 3件、Rust focused、Biome、tsc、generator --check PASS。配布crate8,530,564/10,485,760B。既存index削除なし、basic indexのみ追加。
- 完全再検証前の空き容量が4.1GiBのため、担当cargo停止と対象worktreeでcargo非実行を確認し、標準cargo clean -p katana-render-runtimeで再生成可能な成果物を整理。ソース・stash・worktreeは不変。
- Windows thread PRRT_kwDOSdqU6c6h3bisは後続readでisResolved=trueを確認。APIのエラー応答のみで成否を断定せず、公開返信3999249030と実際のthread状態を再照合して完了とした。
- cargo cleanは14.9GiBの再生成可能な成果物を削除。完全gateログは /tmp/krr-release-check-selective-observer-final.log、全描画比較は /tmp/krr-visual-selective-observer-final.log。実行中であり成功扱いしない。
- Terraのread-only独立レビューはP0/P1/P2指摘なし。native cfgの実挙動不変、Draw.io選択後だけの展開とcache撤去、observer layout presenceと両層回帰を確認。mainも差分と全資産byte一致を独立確認済み。
- 完全gateは通常1016 tests PASS後、coverage22,622行中1未到達でexit1。`cargo llvm-cov report --show-missing-lines`でarchive.rs:57（group_bytesからcompressed range errorを伝播する経路）を特定。helper単独の境界testだけでなく呼出元group_bytesへinvalid groupを与える回帰を補完中。observerの新error helperとfont cfgには未到達なし。
- invalid groupからのerror伝播をtest-only入口で検証し、focused PASS。完全再実行ログは /tmp/krr-release-check-selective-observer-coverage-retry.log。
- リリース前の最新性を再照合: cargo outdated --workspaceは全最新、bun outdatedは新たに@types/node 26.4.1→26.5.1を検出。package.json/bun.lock限定で更新し、JS/型/生成物検査を再実行する（mixed_cjk_repair担当）。
- 再実行は終了コード0で完全PASS。通常/coverage/配布crate各1017 tests PASS（既存ignore1）、22,625行/2,865関数とも未到達0、362files/8,530,750Bのcrate、publish dry-run成功。公開自体は未実施。
- @types/node26.5.1と推移undici-types8.9.0へ更新。TS/Biome/JS33件/bundle検査PASS、bun outdated候補なし。runtime6種も配布元へ再照合して追加更新なし。
- 通常commit済み: f071c24（font cfg/#74）、8d9a75b（observer/#73）、e5035b1（Draw.io/#75）、05b08b3（JS依存/#75）。描画全比較完了後に台帳とterminal pinを固定して通常pushする。

後処理の事前確認（削除未実施）:
- stash 4296adefc438fe12ab5fdfbac234592c2e074225のtracked9pathをmainとLunaで確認。indexmap2.14.1は現2.14.2、rollup4.63.1と各platform lockは現4.63.2、Draw.io31.4.1は31.4.5、ZenUML4.2.1は4.3.0へ置換済み。Justfile/Cargo include/runtime_assets/commonの変更はこれらのversion/checksum参照更新だけ。ZenUML archive/indexは新版generator --checkと完全テストで一致確認済み。
- mainはstash第3parent 4e91bcabff2e03b29603ed905bcad9ea53b90f02も確認。未追跡5filesはDraw.io31.4.1のjs/br/sha256とZenUML4.2.1のjs/sha256のみ。固有実装修正はなく、公開確認後の整理候補。公開前にはdropしない。

最終ローカル結果: Mermaid full/CI・Draw.io full/CIの全工程が終了コード0。29 reports/757 entriesすべてPASS、最小99.01819（Mermaid CI99.59、Draw.io CI99.50）。参照画像・品質基準を変更せず、05b08b3までの修正と最新JS型依存を検証済み。次工程は台帳commit・terminal pin更新・通常push・追加2threadへの返信/解決・新HEAD initial/final review・新HEAD CI。保護mergeと公開は未実施。

## bdd0405 の CI・再レビュー

- [x] 通常push成功、remote HEAD bdd04054d85149fe7318a23eff742f0fb6ff0fa9。pre-push は lefthook の output=failure で just check を無出力実行する。helper stdin待機/CLOSED socketだけでは通信障害と断定できず、親Gitとhook子孫を先に確認する。先の2回の中断は誤診だった。
- [x] P1 3999248718 は返信3999349150、P2 3999248722は返信3999349269を公開し、それぞれ個別resolve成功。
- [x] CI34750907289の4OSとpreflight34750907295はすべてSUCCESS。Linux実coverageは22,591行/未到達0、font_systemも208行/未到達0。macOSとの差はOS別コンパイル対象による。
- [x] initial marker5652641730（2026-09-13T10:13:01Z）で最新HEAD/body digest固定。formal review5190400892でP0/P1=0、下記P2が2件。
- [ ] 3999359327 / PRRT_kwDOSdqU6c6h4Dtr: template_contentsをserializeしないためnested markupが欠落。serialize専任へRED/GREENとIncludeNode/ChildrenOnly双方の回帰を委譲。
- [ ] 3999359329 / PRRT_kwDOSdqU6c6h4Dtt: custom observer rootの外でも座標が重なると交差扱い。observer専任へ祖先関係・intersectionRect/ratioの一貫性と実host回帰を委譲。
- [ ] 修正統合・完全gate・terminal更新・通常push・返信/resolve・新HEAD initial/final reviewを実施する。Ready化・App live操作・公開は未実施。

通常release App activateのread-only dry-runは最終markerが旧HEADのためfail-closed。書込/JWT/IAT生成はなし。PR #72を前提化せず、最新レビュー完了後に再検証する。

P2対応の範囲判定:
- templateは修正前RED→修正後focused5件GREEN、strict clippy/fmt/diff-check PASS。host自身のChildrenOnlyもtemplate専用Documentから選択する。
- observerは実hostでroot内child=true:1、重畳sibling/root自身=false:0・empty rectをRED→GREENで確認。Document rootのviewport経路も回帰追加。
- 独立監査はcontaining-block chainの欠落をP1候補としたが、mainは今回差分が生む後退ではない既存のCSS互換制約として別管理と判断。現行ElementBox/bridgeにCB親情報がなく、今回のDOM sibling除外と別にlayout metadataの横断追加を要する。既存のparity Issue #78へ再現条件・仕様・必要metadata・検証DoDを明記（comment5652697628）。DOM ancestryをCB完全準拠と主張しない。今回の目次追従契約・品質閾値は変更しない。

追加修正の完全ゲート:
- d70393f（template/#75）と9b8098a（observer/#73）へ通常commit。Document root回帰はrootBounds=160x100、intersectionRect=160x20、ratio1までassertする。
- /tmp/krr-release-check-template-root-final.log は終了コード0。通常/coverage/配布crate各1020 tests PASS（既存ignore1）、coverage22,647行/2,867関数とも未到達0。配布crate8,531,741/10,485,760B、publish dry-runも成功。公開自体は未実施。
- cargo outdated --workspace、bun outdatedを再照合し、追加更新候補なし。
- /tmp/krr-visual-template-root-final.log は実行中。Mermaid full/CI、Draw.io basic/official diagrams/examplesまでPASS。完了後の数値はPR/Issue証跡へ記録する。未完了の比較を成功扱いしない。
- 次工程はterminal包含固定、通常push、全描画比較完了確認、固有返信/resolve、新HEAD initial/final review・CI・App admission。その後の保護merge/公開/Issue/cleanupまでを維持する。

## bfcbd52 の完了証跡と追加レビュー

- [x] terminal26f32eaf7319a3e681a957922939a82b9bbf179aへ固定（12契約PASS）、bfcbd5243f243cebd4da832919e53cadac1f789bまで通常push成功。
- [x] template/root指摘へ返信3999428594/3999428648を公開し個別resolve。
- [x] CI34753164108は4OS全SUCCESS、preflight34753164106もSUCCESS。Linuxは22,613行/未到達0。
- [x] /tmp/krr-visual-template-root-final.log 全工程終了コード0。29 reports/757 entries全PASS、最小99.01819（Draw.io CI20件・最小99.50）。Issue #75 comment5652951620へ完了証跡を記録。
- [x] initial marker5652946406（11:21:21Z）で最新HEAD/body固定。formal review5190548074（11:27:49Z）はP0/P1=0、下記P2が2件。
- [ ] 3999483176 / PRRT_kwDOSdqU6c6h4YXF: Acorn移行でevalのno-substitution TemplateLiteralを検査しない。TS gate/test担当へRED→GREENを委譲。
- [ ] 3999483181 / PRRT_kwDOSdqU6c6h4YXK: observer不在でもlayout更新ごとにDOM全snapshotを2回実行。HTML session担当へ軽量判定・snapshot省略とlate observe/metrics/timeout回帰を委譲。
- [ ] 修正後の完全release-check、影響範囲と生成物不変確認、terminal/push/reply/resolve、新HEADレビュー・CI・App admissionを継続する。公開前提・品質基準は不変。

この追加2件の描画証跡引継ぎ条件:
- Terra独立監査とmainのcall site確認により、eval変更はcheck-mode検査器のみ、observer変更はHTML session経路のみと確認。図比較はDiagramV8Runtime→SVG→Playwrightであり、HTML sessionを呼ばない。
- 証跡元SHAはbfcbd5243f243cebd4da832919e53cadac1f789b（757件全PASS）。git diffの全変更pathが、HTML runtimeの上記3files、TS checks/test、docs2files、包含gate2filesの監査済みallowlist内であることを機械照合。現時点7pathすべて一致、生成bundle/manifest・vendor/asset・lock・diagram renderer・共通V8/font/SVG・rasterizer変更なし。
- 最終diffでも同じallowlist照合とbundle --checkを実行する。未知pathや描画入力変更が出たら再利用を拒否し再比較する。HTMLを含む完全release-checkは新差分で再実行するため、品質基準や受入対象の削減ではない。
- mainはprepare timeoutでruntime破棄する経路と、late observe回帰がscroll副作用による偽陽性にならないことを追加確認するよう指示。

追加修正の最終ローカル証跡:
- 7bb3cc8: evalの置換なしtemplateをcooked値で検査。RED→GREEN、import/export/escape/substitution除外と既存literal挙動、Bun4件/型/Biome/bundle --check PASS。生成bundleは不変。
- cdd0e63: observer空時はmetrics/readyだけ更新してsnapshotを省略。unobserve/disconnect、Clickによるlate observe、prepare/refresh例外とtimeout破棄、実scroll回帰を確認。scrollがreadyを準備する偽陽性を排除した。
- /tmp/krr-release-check-eval-observer-final.log は終了コード0。通常/coverage/配布crate各1025 tests PASS（既存ignore1）、22,676行/2,869関数とも未到達0。配布crate8,532,217/10,485,760B、publish dry-run PASS。
- 描画757件の証跡元bfcbd52に対する監査済み非描画path照合と、最新bundle/assets一致検査を満たすため、同じ描画入力/実装の全比較証跡を継承する。公開後のIssue/cleanupまで未完了。

## 61e7ba6 の CI・constructor 入力レビュー

- [x] 通常push成功。terminal100ce22913a3674bddac618b02ab97b54893c6a1の包含契約12件PASS。
- [x] eval/observer指摘へ返信3999552141/3999552215を公開し個別resolve。
- [x] CI34755720866の4OSとpreflight34755720863はすべてSUCCESS。Linux22,642行/未到達0、font system208行/未到達0。
- [x] initial marker5653114163（11:57:47Z）、formal review5190630703（12:06:27Z）。P0/P1=0、次のP2が2件。
- [ ] 3999569312 / PRRT_kwDOSdqU6c6h4nNh: 不正rootMarginの黙示的0px fallbackをSyntaxErrorへ修正。
- [ ] 3999569316 / PRRT_kwDOSdqU6c6h4nNk: threshold数値変換と検証。有限の範囲外はRangeError、WebIDL doubleのNaN/InfinityはTypeError。空配列/defaultと昇順を維持する。
- [ ] dom_bootstrap.js実装担当と実runtime回帰テスト担当を分離。統合後の完全gate・包含pin・push・固有返信/resolve・最新HEADレビューを継続する。

今回の入力検証はHTML専用経路。最終diffが既存の監査済みallowlist内でbundle/assets不変かつ完全gateがPASSなら、bfcbd52の描画757件証跡を引き継ぐ。Ready・App live操作・公開はまだ実施していない。

- constructor回帰は実runtimeでfocused PASS、対象clippy/fmt/diff-check PASS。数値NaN/±Infinity、疎配列、boxed BigInt、数値文字列とrootMarginの4値getterを含む。初回REDの終了ログは実装との同時実行により捕捉できず、RED確認済みとは主張しない。
- 独立監査のSet/TypedArray入力は既存未対応で今回の後退ではないため、Issue #78 comment5653211288へHTML parity残件として記録。公開DoDへ追加しない。
- `/tmp/krr-release-check-constructor-final.log` で完全gateを実行中。
- [ ] 停止原因となるcommit skill冒頭の承認待ち記述を、承認済み範囲では検証報告後に継続するルールへ是正。.codex/skills/commit_and_push/SKILL.md と .agents/skills/commit_and_push/SKILL.md を別担当で限定修正する。App操作固有のfresh承認・品質gateは不変。この2pathは実行コードでも描画入力でもないskill文書のため、内容確認後に非描画証跡allowlistへ追加する。

- [/] commit skill両版の本文冒頭2行を修正しmainも独立diff確認。承認済み範囲では検証報告後に通常commit/pushを続行し、未承認公開/範囲拡大/App固有fresh承認は維持する。2pathは文書のみと確認し、描画証跡allowlistへ追加。diff-check PASS。skill quick_validateは両版の既存frontmatter名commit_and_pushのhyphen-case違反で拒否し、本文変更に関係しない既存命名問題までPASSと主張しない。
- 初回完全gateはconstructor_validation_sourceの関数長33/30でexit101。事前の対象crate clippyはrepo固有の明示lintを有効にしていなかった。fixtureの責務分離と実入口just lint/ast-lint/fmt-checkへ是正し、閾値不変で再実行する。
- fixtureをprelude/正常値/不正値の責務で分割し、関数8/14/13/15行・ファイル277行へ収めた。just fmt-check / lint / ast-lintとdiff-checkはすべてPASS。完全gate再実行ログは `/tmp/krr-release-check-constructor-retry.log`。
- 最終候補の依存再照合も成功: cargo outdated --workspace全最新、bun outdated候補なし。既存LatestVersionClientによる6配布元照合もMermaid12.0.0/Mermaid ZenUML1.0.0/ZenUML Core4.3.0/Draw.io31.4.5/MathJax4.1.3/PlantUML1.2026.8でcurrent=latest。再更新の必要なし。
- 再実行は通常1026件PASS後、ASTがテスト本体の33行を検出してexit101。先の担当報告は起動行だけから成功を推定しており誤りだった。テスト本体/正常値/不正値/assert helperを9/16/17/5行へ分離、ファイル296行。raw AST直接テストとCARGO='rtk proxy cargo' just ast-lintの両方で実テスト1件PASSを確認した。新しい完全gateログは `/tmp/krr-release-check-constructor-ast-final.log`。
- 最終完全gateは終了コード0。通常/coverage/配布crate各1026件PASS（既存ignore1）、22,676行・2,869関数とも未到達0。配布crate362files/8,533,227B（上限10,485,760B）、publish dry-run成功。最終diff11pathの非描画allowlist照合・未追跡0・生成bundle/assets整合も成功し、bfcbd52の757件全PASS証跡を継承。実公開は未実施。
