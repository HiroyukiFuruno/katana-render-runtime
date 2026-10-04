# katana-diagram-renderer Agent Rules

## Branch Policy

- 公開配布（crates.io）、release tag、公開CLI、公開API、package metadata に影響しない変更は `master` 直接作業でよい。
- 公開配布や release に影響する変更は、作業前に branch 方針を確認する。
- ユーザーが push を明示した場合は、ローカル commit で止めず、通常の `git push` まで実行する。
- pre-push が失敗した場合は回避せず、失敗した検査を修正してから再度 push する。

## レビュー観測と継続

- レビュー待機判断ではPR Issueコメント、formal reviews、reviewThreadsを全ページ取得する。Issueコメント不在だけで未応答と判断しない。取得エラーは観測失敗として復旧し、指摘があれば修正へ進む。
- 分離できる依存更新・指摘修正は非重複担当へ委譲する。既に依頼された範囲は再承認待ちで止めず、仕様変更・新たな権限が必要な判断だけ相談する。
- Draft → 最新HEADのreview取得・修正・各threadへのreply/resolve → Ready → required checks → 保護mergeの順序を守る。一括resolveとCIのみの完了判定は禁止する。
- リリース直前にRust/JSの直接・推移依存とlockfileを再確認し、更新後の完全ゲートを通す。公開とIssue整理・安全なcleanupを確認するまで完了扱いしない。

## Release Inclusion Gate

- ユーザーが特定の修正を指定versionへ抱き合わせるよう指示した場合、release対象commitを `scripts/release/verify-release-target.py` の `REQUIRED_RELEASE_COMMITS` に固定する。
- `release-target-check`、PR作成、mergeの各時点で、release branchのHEADが全必須commitを含むことを `git merge-base --is-ancestor` で機械検証する。
- 別release branchのversion bump、tag、GitHub Release、crates.io公開が成功していても、必須commitを含まない場合は指定releaseの完了として扱わない。

## PR Review Gate

- Pull Request は必ず Draft として作成する。Ready PR を直接作成してはならない。
- 初回 Draft の前に `just draft-review` を実行する。`just check` と pre-push は標準品質laneだけを実行し、ローカルHigh reviewは必要時に `just local-review` で明示する。
- Draft で最新 HEAD に対する `@codex review` と自己レビューを実施し、GitHub APIで Issue コメント、formal reviews、review threads を全ページ取得する。初回と再レビュー2回を通常目安とする。
- 各指摘を個別に分類し、P0/P1は必ず修正、P2/P3は要件・互換性・DoDへの影響を根拠付きで判断する。不要な改善は理由を記録し後続patchへ送れる。修正後は自己レビュー、関連する静的検査と回帰確認を行い、push、該当threadへのreply、threadのresolveを個別に完了する。静的解析だけで実動作を保証したとは扱わない。
- レビュー回数を理由に未解決P0/P1、DoD違反、最新HEADのPR review、個別reply/resolve、required native checks、expected SHA付きprotected mergeを省略しない。
- 最新 HEAD のレビュー結果、未解決thread 0、Issue/DoD、required CI checksを確認してからReady化する。Ready化後は保護されたPR mergeを、merge直前に再取得したexpected HEAD SHAで実行し、人間による直接master更新やadmin bypassを使わない。

## Orchestration Gate

- main agent は司令塔として、設計、ハーネス、担当分離、統合レビュー、最終ゲートを担う。分離可能なreview指摘修正をmainが直列実装しない。
- 変更ファイルと責務を先に棚卸しし、同時実行枠の範囲で1ファイルまたは非重複責務ごとにsubagentへ並列委譲する。空き枠を放置して直列化しない。
- subagent起動前に、最新のユーザー指示と利用可能モデルを確認する。利用不可のモデルを選ばず、限定実装はLuna、複雑な設計・統合分析はTerraへ切り替える。
- main agent はsubagent結果を鵜呑みにせず、追加fixture、差分レビュー、完全ゲートで統合判定する。
