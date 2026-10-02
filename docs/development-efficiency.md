# 品質を維持した検査の順序

高コストな完全検査の前に、提案差分が Issue の要求と一次仕様に合っているかをローカルで確認する。
テスト成功だけでは assertion の期待値が正しいことを証明できないため、仕様・実際に観測すべき動作・assertion を対照する。

## ローカルレビュー

`just check` は最初に `scripts/hooks/local_review.py` を実行してから既存の品質 lane を実行する。
pre-push も `just check` を呼ぶため、この順序を共有する。review receipt が有効でも品質検査は省略しない。

```bash
rtk proxy env REVIEW_ISSUE=89 just check
rtk proxy python3 scripts/hooks/local_review.py --issue 89 --print-input
rtk proxy python3 scripts/hooks/local_review.py --issue 89 --check-receipt
```

Issue は `--issue`、`REVIEW_ISSUE`、base からの branch commit の一意な `Refs #N`、直近のレビュー入力、既存 receipt の順で解決する。
複数の Issue が推定される場合は黙って古い receipt を選ばず、担当 agent が対象を明示する。
89を既定値にはしない。Issue 本文・title・state・URL を毎回取得し、コメント時刻だけの変更では失効させない。
追加の要求ファイルは `--requirements <repo内path>`、`REVIEW_REQUIREMENTS`、同じIssue集合を持つ直近レビュー入力、既存receiptの要求pathの順で解決する。
標準 `just check` でも同じ要求を保持し、毎回その内容を読み直す。保存pathの不正・欠落は拒否し、新Issueへ切り替えた際は旧Issueの追加要求を引き継がない。

初回または入力変更後は、既存認証の Codex CLI を `gpt-6.1-sol / high`、read-only、structured schema で実行する。
追加 API key は不要で、商用コード、GitHub、Cargo の操作を review に許可しない。
子レビューには `KRR_LOCAL_REVIEW_ACTIVE=1` を渡し、再帰的な review/check を拒否する。
CI=true または GITHUB_ACTIONS=true の環境ではローカル CLI を起動せず SKIP を明示する。
これは既存 cloud review／native CI の代替成功証拠ではない。

review は次の6観点を出典・短い引用・観測結果と結び付ける。

- Issue の要求と変更範囲
- 一次仕様と assertion の期待値
- 修正前後の回帰入力の意味
- 公開 API と互換性
- 安全性と品質対象・閾値の維持
- 影響が不明な場合の完全検査への fallback

P0/P1 と、要件・互換性・DoD を妨げる P2/P3 は未対応のまま PASS にしない。
非 blocking 指摘の採用／不採用には具体的な理由が必要である。
review は提案差分の適合を評価する。CI、完全品質検査、merge、公開、Issue close は通常の後工程として pending を明記する。
後工程が未実行であることだけを pre-check の失敗理由にしない。

## receipt の拘束と失効

`tmp/local-review/receipt.json` は base commit、source の path/content/mode、隠れ staged 版、Issue 本文、追加要求、model、prompt、schema に結び付く。
Justfileの有効coverage閾値、threads、RUSTFLAGS、Cargo command、並列数と、限定したCargo対象・compiler設定も拘束する。
`just --set` の親有効値を同じshellからreviewと品質laneへ渡すため、レビューだけ既定値へ戻ることはない。
通常targetとcoverage専用target/build-dirを絶対pathへ正規化する。同じpathの相対／絶対指定は同一入力になる。
同じ既定値を明示するだけなら再利用でき、実際の値が変われば失効する。全環境変数や認証情報は採取しない。
tracked／unstaged／untracked／deleted を含み、削除は base のファイル一覧から保持する。
通常の staging／commit だけで内容が同じ場合は再利用できる。working tree と異なる staged 内容は別に拘束する。
レビュー開始前後と再利用時に入力を再取得し、source や要求が変われば古い PASS を受け入れない。
CLI には変更一覧と入力 digest を渡し、全ファイルの hash 一覧を無駄に読み込ませない。

receipt の digest と保存 structured 原本も照合する。破損・別入力・未知の field・重複 JSON key・CLI エラーは拒否する。
FAIL 時の指摘は `tmp/local-review/last-review.json`、対応入力は `last-input.json` に残り、修正へ引き継げる。
初回FAILでreceiptが作られなくても、structured reviewの入力digestと照合した `last-input.json` から同Issueの要求pathを保持する。
保存入力を検証できない場合は、要求を黙って除外せず明示設定を求めて拒否する。
`--check-receipt` は検証だけを行い、新しい Codex review を起動しない。

同じローカルユーザーが書き込める evidence なので、これを署名済みレビューや敵対的な改竄への認証として扱わない。
JSON 整形式や CLI 終了コード0も意味的な正しさの根拠にはしない。独立 review と完全品質ゲートは維持する。

## 軽量検査と影響試験の選択

変更に対応する既存 Python／JS 単位テストや契約を先に実行し、その入力と期待値を review する。
実行しなかった検査を PASS と記録しない。選択したテストは早期発見用であり、既存の完全検査を置き換えない。
read-only sandbox内で一時fileを作れない検査は、親が保存した同入力の実検証証拠を照合できる。
実行前後のsource snapshotと今回のsnapshot digest、有効gate設定digest、実command、exit0、原本logのhash一致が必要で、単なる成功報告では代替できない。
これはsandbox内の重複実行を避けるための軽量証拠であり、完全品質ゲートは後工程で実行する。

2026-10-02 に [cargo-impact](https://github.com/asmuelle/cargo-impact) 0.5.0 の公式配布物の SHA を検証し、PR99 の f381→509 差分を実際に解析した。
所要時間は2.504秒だったが、変更された JavaScript 2ファイルが解析対象から欠落した。
Rust テストの候補16件に対し、報告は tmp 文書の単なる keyword 照合1件（`doc_drift_keyword`）で、実行すべきテスト集合を返さなかった。
評価原本は `tmp/cargo-impact-evaluation/pr99-window-lifecycle-impact.json` に保持する。
rust-analyzer 接続にも help stub があり、完全な影響判定を確認できなかった。
したがって、この結果を試験省略の oracle には採用しない。JS、workflow、生成 asset、動的 callback など解析対象外や不明な変更は完全検査へ戻す。

## CI と公開の区別

Issue89で導入した同一 repository／PR／base／head／workflow／run／job／step の二読検証は、成功済み CI 品質証跡を release-preflight が再利用する別の仕組みである。
ローカル review receipt は品質検査済みという証拠ではなく、高コスト検査を始める前のレビュー結果だけを再利用する。
Draft review、各指摘の修正／reply／resolve、native required checks、保護 merge、公開の手順は変えない。
