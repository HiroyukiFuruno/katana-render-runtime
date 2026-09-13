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
