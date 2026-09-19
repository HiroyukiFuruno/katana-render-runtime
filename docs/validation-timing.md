# Validation timing audit

Issue #84 の第一工程として、明示した1コマンドの実行時間と終了状態を
JSONへ記録する軽量CLIを追加した。

```text
rtk proxy python3 scripts/validation-timing/measure.py \
  --output tmp/validation-timing/check.json \
  --check-id release-target \
  --input Cargo.lock --input Justfile -- \
  python3 -m unittest scripts/release/verify_release_target_test.py
```

コマンドは引数配列のまま `subprocess.run(..., shell=False)` へ渡す。
`--check-id` は集計時の安定した検査識別子で、1レコードにつき
`invocation_count: 1` を記録する。`--input` で明示した通常ファイルは内容とパスを
SHA-256へ含め、`inputs` と `input_digest` として記録する。指定した入力が欠損・通常
ファイルでない場合は、対象コマンドを起動せず終了コード2で失敗する。
記録項目は開始・終了時刻（UTC）、monotonic経過秒、終了コード、検査識別子、引数配列、
入力一覧と入力digest、
開始/終了HEAD、開始時cwd、開始時dirty状態、OS、architectureである。標準出力・
標準エラーはJSONへ記録せず、計測対象プロセスから通常どおり継承する。
環境変数は収集しないが、引数配列は保存するため認証情報を引数へ指定しない。
実行できないコマンドは終了コード127とする。実行されたコマンドの
非ゼロ終了コードはそのまま記録・返却し、signal終了の子プロセスはrawの負数をJSONへ
記録したうえでCLI終了コードを `128 + signal` に変換する。

この記録は工程ごとの実測と監査用の観測データであり、成功した検査のskip根拠や
品質ゲートの代替には使わない。OS、architecture、HEAD、コマンド引数、入力状態を
比較できる形にするための第一段階であり、証跡の署名・信頼issuer・再利用判定は
未実装である。Git情報を取得できない場合の `head_at_start`/`head_at_end` は
`unknown`、dirty状態は `null` として記録する。

## 検証

```text
rtk proxy python3 -m unittest discover -s scripts/validation-timing -p 'test_*.py'
rtk proxy python3 scripts/validation-timing/measure.py \
  --output tmp/validation-timing/self-test.json \
  --check-id validation-timing \
  --input scripts/validation-timing/measure.py \
  --input scripts/validation-timing/test_measure.py -- \
  python3 -m unittest discover -s scripts/validation-timing -p 'test_*.py'
```

第二コマンドは既存のunittestコマンドを実際に計測する確認用である。
短縮率、導入前後の比較、CI/releaseでの再利用はまだ実測・実装していない。
Issue #84 のDoD全体は未完了である。
