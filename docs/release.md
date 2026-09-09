# リリース手順

## 方針

`katana-render-runtime` は実装の本体として公開し、`katana-diagram-renderer` への互換ルートは公開対象に含めません。

`release/vX.Y.Z` ブランチから `master` へ取り込み依頼（Pull Request）を作る。
その取り込み依頼（Pull Request）では通常の品質ゲート（quality gate）とリリース前検査を必須にする。
PR 作成前には `lefthook run pre-pr` を実行し、対象版番号（version）より前の完了済み OpenSpec change が `openspec/changes/archive/` に移動済みであることを確認する。同じ版番号の change に downstream 公開後の作業が残る場合は、その change を release PR の archive 条件に含めない。
取り込み（merge）後は自動実行基盤（GitHub Actions）がタグ（tag）、GitHub リリース（GitHub Release）、crates.io 公開を実行する。

## 必須検査

GitHub のブランチ保護（branch protection）では、少なくとも次を必須検査（required check）にする。

- `Test and Build (macos-latest)`
- `Test and Build (ubuntu-latest)`
- `Test and Build (windows-latest)`
- `preflight`

## リリース前検査

`release-preflight` は `release/v...` ブランチの取り込み依頼（Pull Request）で `just release-check` を実行する。
`just release-check` は、release 予定版番号（version）より前の OpenSpec change が active 側に残っている場合に失敗する。
内容は次の通り。

- 版番号（version）が GitHub Release / remote tag 上の自然な次版であること。`v0.3.0` は rename release として KDR の版番号を引き継ぐ
- ユーザーが対象versionへ含めるよう指定した全commitをrelease branchが祖先として含むこと
- 対象タグ（tag）が remote 上の既存タグを上書きしないこと
- 対象版番号（version）が crates.io に未公開であること
- 整形確認（format）、静的検査（lint）、単体テスト（unit test）、抽象構文木検査（AST lint）
- KatanA UI 依存の混入検知（dependency leak）
- カバレッジ（coverage）。行カバレッジ（line coverage）100%、未到達行（uncovered line）0
- runtime bundle の同期、TypeScript 型検査、runtime asset の checksum 確認
- `Cargo.toml` の版番号（version）と branch 版番号（branch version）の一致
- 作業領域（workspace）内部依存の版番号（version）一致
- `katana-render-runtime` の梱包（package）と公開の事前実行（publish dry-run）
- `katana-render-runtime-cli` の梱包（package）収録対象確認

## 依存更新の実行メモ

依存更新は `just depends-update-all` で実施し、score 99・完全check・coverageまでの終了結果を確認する。更新ファイルが生成されたことだけで成功扱いにしない。

runtime資産の定数更新・圧縮資産生成後は既存の `just fmt` でRust生成コードを整形してから検証する。checksumやURLの長さでrustfmtの改行位置が変わるため、更新スクリプトの文字列置換だけを整形済みとみなさない。

更新・bundle生成後は `just check` と `just coverage` を先に通してから、時間のかかる全参照画像生成・比較へ進む。Clippyの合格だけではASTのファイル責務・行数制限の合格を保証しないため、テスト追加時も既存の `just ast-lint` を確認する。テスト本体、入力fixture、描画bundle、幾何検証helperを責務で分け、行数上限やテスト除外を変更して回避しない。`runtime-asset-script-test` は一括更新レシピの必須工程とこの順序を検証し、検査の削除・後回しを検出する。

別ファイルへ抽出したテストhelperにもcoverageを確認する。幾何検証helperなら、有効な座標変換だけでなく不正SVG・属性欠落・比較対象欠落を拒否する入力をテストし、検証側が誤って成功しないことも保証する。未到達箇所は計測結果から特定し、ファイル名変更や除外設定で隠さない。

coverageのレポートだけを読む場合は `cargo llvm-cov report` を使う。`report` を省くとテストを再実行する。`--no-clean` の増分計測は原因の切り分けに限定し、修正前後の異なるコード・feature構成の記録を最終証跡へ混ぜない。最終合否は既存の `just coverage` が行うclean後の全体計測で確認する。

通常の行表示が到達済みでも未到達数が残る場合は、`cargo llvm-cov report --text --show-instantiations` でジェネリックの型・const引数ごとの実体を確認する。例えば要素数1の検証が不正入力の早期returnしか通っていなければ、同じ要素数の正常入力も検証する。クリーン計測の未到達を、根拠なく古い計測記録と判断しない。

worktree間でbuild cacheを共有するために `CARGO_TARGET_DIR` を指定するときは、既存の `KRR_BIN` も同じtarget配下の `debug/krr` へ指定する。`KRR_BIN` の既定値は現在のworktreeの `target/debug/krr` なので、片方だけ変えるとbuild成功後のfixture描画が「実行ファイルなし」（exit 127）で失敗する。この場合はscoreを変更せず、両方の参照先を揃えて完全コマンドを再実行する。

環境変数付きの検証コマンドは `rtk proxy env ... just ...` で実行する。`rtk env` は環境変数を表示する別コマンドなので、環境設定・コマンド実行用の `env` と取り違えない。`RUSTFLAGS='-D warnings'` などの値とfeature指定も揃え、不要な別profileの再ビルドを避ける。

subagentへのworktree指定は指示文だけで済ませず、各実行の作業ディレクトリと編集先を絶対パスで固定し、作業前後にbranch・対象diffを照合する。レシピを検査するテストも実行時cwdへ依存せず、`import.meta.url` から所属repositoryのファイルを参照する。別worktreeの旧レシピを検査した結果を現行変更の証跡へ混ぜない。

長時間実行ではsession IDとログ保存先を記録し、観測のtimeoutだけで再起動しない。同じ実行の終了codeを確認してから、失敗原因を修正して再実行する。lockfileを復元した後は `bun install --frozen-lockfile` で導入済みツールも一致させ、設定schemaとCLIのversionずれを持ち越さない。

Draw.ioの `sketch` / `comic` の差分は、図形解釈やcrop補正の前に描画条件を確認する。KRRの共有runtimeは `Date.now` と `Math.random` を固定するため、公式rendererもvendor読込前に同じ条件を初期化する。`runtime-asset-script-test` は実際の共有runtimeと公式側の時刻・乱数列を照合し、設定の乖離を検出する。乱数だけの差か、canvas寸法・origin・path・文字の差かは、同条件の独立出力で切り分ける。

crop補正は1枚の見た目から一般式を推測せず、公式exportで線幅などを変えた入力を検証する。別のtop-level図形や、親の外へはみ出す要素を切り落とさない回帰も追加する。参照画像は正しい入力条件の公式rendererから生成し、スコア下限を変えず全カテゴリを再検証する。

線幅は偶数・奇数の両方で確認する。奇数幅のcrisp描画には親の `translate(0.5,0.5)` が含まれるため、layout frameの原点と変換済みpaint bboxを混同しない。既存のcrisp移動除去helperを使い、一般の座標変換や子要素のはみ出し保護は保持する。全件比較ではスコアだけでなく、入力・公式PNG・出力PNGのファイル名集合が一致することも確認し、出力欠落を成功に数えない。

SVGが同一なのにPNGの文字が変わる場合は、参照側のfont読み込みも確認する。`document.fonts.ready` はstylesheet内のfont-faceが登録される前に解決する場合があるため、それ単独では十分でない。公式Draw.ioのcaptureは入力の `fontSource` stylesheetを明示ロードしてからfontsの完了を待ち、ロード失敗を成功扱いしない。font名の指定、FontFaceSetの登録・load状態、実画像の文字を照合し、参照側のfallbackに合わせてKRRの文字描画を変更しない。

## HTML 系プレビュー前提条件（release contract）

`katana-render-runtime` の HTML/CSS の interactive preview は、外部ブラウザや WebView を経由せず、プラットフォームの system font fallback に依存する設計です。
このため、日本語文字を表示する場合は、対象ホストに日本語対応のシステムフォントが存在することを前提とします。
前提条件を満たさない環境では、文字が tofu（□）になる可能性があります。

この条件は `v0.4.3` のリリース契約として扱い、release checklist に明記して確認を維持してください。

`katana-render-runtime-cli` は `katana-render-runtime` を先に公開しないと crates.io 上で依存解決できないため、
取り込み依頼（Pull Request）時点では `katana-render-runtime` を事前実行（dry-run）し、CLI は収録対象確認までに留める。
公開順序は `katana-render-runtime`、`katana-render-runtime-cli` の順に固定する。

## 公開順序

取り込み（merge）後の `Release` ワークフロー（workflow）は次の順で動く。

1. `just release-target-check`
2. `just release-verify`
3. リリースタグ（release tag）作成
4. GitHub リリース（GitHub Release）作成
5. `katana-render-runtime` を crates.io に公開
6. crates.io で `katana-render-runtime` が見えるまで待機
7. `katana-render-runtime-cli` を crates.io に公開

## 必要な秘匿値

自動実行基盤（GitHub Actions）には `CARGO_REGISTRY_TOKEN` が必要。
値は crates.io の API トークン（API token）を使う。

ユーザーが実行する登録コマンド:

```bash
cd /Users/hiroyuki_furuno/works/private/katana-render-runtime
gh secret set CARGO_REGISTRY_TOKEN
```

`Cargo` は crates.io のトークン（token）を `CARGO_REGISTRY_TOKEN` 環境変数で受け取れる。
トークン（token）は秘匿値として扱い、リポジトリ（repository）に保存しない。

参考:

- https://doc.rust-lang.org/cargo/reference/config.html#credentials
- https://doc.rust-lang.org/cargo/commands/cargo-publish.html
