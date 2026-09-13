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
- [ ] 最終差分の統合レビュー、完全release-check、描画スコア、依存最新性を確認
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
