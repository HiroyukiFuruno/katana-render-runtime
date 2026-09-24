# PR #72 復旧台帳

ユーザーDoD: 開発flowの是正を実装・検証し、v0.4.20公開を進める。#64は公開動作を変えないflow修正でversion bumpなし。PR #77のRust/JS依存更新・品質ゲートと並行実行する。

- [/] コメント欄だけを見た未応答判定を撤回。formal reviews全233件、threads全177件を取得し未resolve13件を確認。
- [x] writer input上限P1: GitHub.com公式上限25件に対し11件で有効。変更せず根拠返信3997363199、thread個別resolve確認。
- [x] dispatcher全head予算・source failure barrier P1（全governed heads、実early保存数、非文字列shaも回帰）
- [x] direct dependency解決P2（metadata全featuresのdirect package ID、missingをfail-closed）
- [x] initial対応前final投稿のskill是正（main、両create PR skill）
- [x] 残P2（ready最終review後identity fence、cleanup repo束縛、retry Latest、運用cap文書）を修正・回帰検証
- [ ] outdated含む各threadの根拠返信・個別resolve
- [ ] 完全automation testsと差分レビュー、push、最新HEAD/body initial/final review
  - just check PASS: Rust1055件＋ignored1、hooks94/release33/review493、AST/Lint/Biome/typecheck/checksum。独立QA指摘修正後の結果。global skill107件PASS。
- [ ] bootstrap専用App gate→Ready→保護merge→finalize・smoke→verify
- [ ] PR #77はIssue #75どおり独立公開を進める。#72統合を前提に追加せず、既存App-only保護merge経路と公開契約の整合を確認する

禁止: timeout/score/coverageの緩和、無条件resolve、通常CLI/admin merge、token露出。外部待機は観測根拠を確認し、非競合作業を止めない。
