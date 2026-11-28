# リリース手順

## 方針

`katana-render-runtime` は実装の本体として公開し、`katana-diagram-renderer` への互換ルートは公開対象に含めません。

`release/vX.Y.Z` ブランチから `master` へ取り込み依頼（Pull Request）を作る。
その取り込み依頼（Pull Request）では通常の品質ゲート（quality gate）とリリース前検査を必須にする。
PR 作成前には `lefthook run pre-pr` を実行し、対象版番号（version）より前の完了済み OpenSpec change が `openspec/changes/archive/` に移動済みであることを確認する。同じ版番号の change に downstream 公開後の作業が残る場合は、その change を release PR の archive 条件に含めない。
取り込み（merge）後は自動実行基盤（GitHub Actions）がタグ（tag）、GitHub リリース（GitHub Release）、crates.io 公開を実行する。

## 公開候補のコードフリーズ

1. リリースに含める修正、依存関係、lockfile、生成資産を確定し、自己レビューを完了する。長時間の検証を始める前に `just release-target-check` を実行する。
2. clean な作業ツリーで、候補のコミットSHA、依存更新の証跡、検証の出力先を台帳に記録し、その候補をフリーズする。性能測定・描画比較・完全品質ゲート・PRレビュー・CIの結果を候補のSHAに対応付ける。測定が終わるまで作業ツリーを変更しない。
3. レビューで必須の修正が出た場合や既存の公開条件が失敗した場合は、変更内容と影響範囲を確定してフリーズをやり直す。再ビルドが必要な修正を行うときは、適合を確認できた依存更新も同じ候補にまとめる。新しい版が見つかったという理由だけで、進行中の測定と公開候補を更新しない。
4. 候補を更新したら、影響を受ける検証を実行する。同一条件・同一成果物と既存の検証器で証明できる結果は再利用する。必須の品質条件と最新SHAのレビュー・CIを確認してから、保護されたマージへ進む。

完全検査より前に終える実装、検証基盤の修正、個別確認は作業台帳の該当項目に `blocks_full_quality_gate: true` を付ける。全項目の完了と証跡を記録し、自己レビューを済ませてから完全検査を起動する。CI・マージ・公開など、完全検査より後の工程はこの指定の対象外とする。

Linuxで検査環境に依存する失敗を直す場合は、固定した公開候補を手動検証に渡す。Rustのツールチェーンとコンポーネント、Bun、uv、契約テスト用RTKを必須CIと揃え、共通の契約検査を使用する。失敗が続く場合は同じスクリプトの `--collect-failures` で全スイートを一度ずつ実行し、修正をまとめてから再検証する。必須Linux CIはこの契約検査をRustのコンパイルや描画比較より先に実行する。CIのstep時間と再実行回数は、開発フローの改善前後で比較する。

### PRレビューのコードフリーズ

- 初回レビューで確認したP0とP1は、このPRで修正する。初回のP2とP3は、公開条件と影響範囲を確認して扱いを決める。
- 2回目以降に初めて見つかったP0は、このPRで修正する。新規のP1・P2・P3は、再現条件と受入条件を後続Issueに残す。小さな指摘は、個別対応の明示指示がなければ関連Issueにまとめる。
- 後続対応する指摘は、該当スレッドへIssueのリンクを返信してから解決する。初回から残るP1、公開条件の未達、最新コミットのレビューと必須CI、保護マージは引き続き公開前に確認する。
- 指摘に再現用コードやコマンドを添える場合は、コードブロックに記載する。

## 必須検査

GitHub のブランチ保護（branch protection）では、少なくとも次を必須検査（required check）にする。

- `Test and Build (ubuntu-latest, linux64)`
- `Test and Build (mac-arm64)`
- `Test and Build (windows-latest, win64)`
- `preflight`

macOSのCIではApple Siliconの `macos-latest` を使う。ローカルのmacOSがCIと同じメジャーか1つ先で、同じPR・ベース・候補コミットの証跡が検証を通過した場合、CIは該当するmacOS検査を省略できる。証跡が有効でない場合はCIで検査する。

v0.4.24のHTMLでは、macOS 26のCIで固定Chromium観測器が描画済み書体のフェイスを特定できない場合がある。[同じ条件を使ったCI診断](https://github.com/HiroyukiFuruno/katana-render-runtime/actions/runs/46114338296) で、フォントファイルのSHA-256を取得した20件のうち4件で確認した。macOS 27のApple Siliconで全84条件の画像が90点以上かつ出力フォントが一致し、同じPR・ベース・候補に対応する有効な保守者証跡がある場合に限り、macOS 26のCIでその結果を使う。証跡の取得や照合に失敗した場合はCIで同じ84条件を実行する。この証跡は暫定版の描画・書体判定に使い、後続版の時間・メモリ測定には使わない。

## リリース前検査

`release-preflight` は、同一リポジトリ内の `release/v...` ブランチから作成された取り込み依頼（Pull Request）では、`CI` が同じ repository / PR / base SHA / head SHA で生成した Ubuntu 品質証跡を検証してから、`just release-preflight-check` を実行する。検証対象は同一 workflow run attempt の Ubuntu job と必須 step であり、APIから証跡を二度取得して一致を確認する。実行中のCIは上限付きで待機する。証跡の欠落・重複・不一致・failure・再取得中の変化は失敗として扱い、古い成功runへフォールバックしない。

この経路では `check` と `coverage` を再実行せず、成功済みの同一HEAD CI証跡を再利用する。Ubuntu CI側で release 用の runtime asset script test、automation contract test、runtime package asset check を実行する。品質条件は維持し、行カバレッジ100%、未到達行0、描画スコア下限99を適用する。Ubuntu CIのcoverage工程には45分の上限があり、timeoutまたは失敗時にはディスク使用量・プロセス一覧・coverage summaryの診断を記録する。`workflow_dispatch` による手動実行は証跡再利用を行わず、従来どおり `just release-check` で完全な品質検査とリリース固有検査を実行する。

`just release-check` はローカルの完全検査である。PR経路の `just release-preflight-check` は、リリース固有検査を行う。いずれもrelease予定版番号（version）より前のOpenSpec changeがactive側に残っている場合は失敗する。リリース前検査の内容は次の通り。

- 版番号（version）が GitHub Release / remote tag 上の自然な次版であること。`v0.3.0` は rename release として KDR の版番号を引き継ぐ
- ユーザーが対象versionへ含めるよう指定した全commitをrelease branchが祖先として含むこと
- 対象タグ（tag）が remote 上の既存タグを上書きしないこと
- 対象版番号（version）が crates.io に未公開であること
- リリース固有のOpenSpec archive確認
- `katana-render-runtime` の梱包（package）後のライブラリ検査、公開の事前実行（publish dry-run）とpackage収録確認
- 完全品質検査（`just release-check` または再利用するUbuntu CI証跡）では、整形（format）、静的検査（lint）、単体テスト（unit test）、抽象構文木検査（AST lint）、KatanA UI依存の混入検知（dependency leak）、runtime bundle同期、TypeScript型検査、runtime asset checksum確認を行う
- カバレッジ（coverage）は行カバレッジ（line coverage）100%、未到達行（uncovered line）0を維持する。描画比較の最低スコア99も維持する
- `Cargo.toml` の版番号（version）と branch 版番号（branch version）の一致
- 作業領域（workspace）内部依存の版番号（version）一致
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

検証ログの要約にテスト名やpanicが出たことだけで、そのテストの失敗と判断しない。終了code、`test result`、coverageの未到達行を確認する。要約で原因が欠落した場合は、元の実行終了を確認してから `rtk proxy env CARGO='rtk proxy cargo' just coverage` で同じ品質条件の生ログを取得する。実行中のcoverageとの二重起動はしない。

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
8. 公開済み GitHub Release を再確認し、clean・統合済み・未ロックの release branch / worktree だけを整理

cleanup は `scripts/release/cleanup_release_state.py` が担当する。
GitHub Release が未公開、対象 branch が未統合、worktree が dirty または locked、対象が default branch のいずれかなら削除せず失敗する。
local release branch tip と remote-tracking（remote）release branch tip が共存する場合は、両方が default branch に統合済みであることを worktree 除去・branch 削除の前に検証する。local に未push commit がある場合は、何も削除せず fail-closed で失敗する。
ローカルで再実行する場合も、対象branchを明示して同じ安全条件を使う。

```bash
python3 scripts/release/cleanup_release_state.py \
  --version vX.Y.Z \
  --release-branch release/vX.Y.Z \
  --remote origin \
  --default-branch master
```

Issue起点と依存更新証跡の書式は [Issue起点の変更契約](issue-driven-workflow.md) を参照する。

## 実HTMLの初期描画と終了の確認

HTMLの性能修正では、対象原本のSHA-256を固定して次を実行する。

```bash
rtk proxy just html-first-frame-check '/path/to/input.html' '<input-sha256>' 'tmp/html-first-frame.json'
```

配布と同じ最適化releaseビルドを使用し、ビルド時間を除いて1280×900・scale 1の
初期フレームが3秒未満で得られることと、実際のsession closeを
プロセス全体の60秒上限で検証する。終了コード、フレームの寸法・バッファ、
入力・実行ファイルのハッシュ、CPU時間、残存プロセスの有無をJSONへ記録する。
タイムアウト、close失敗、実行中の入力・バイナリ変更は失敗となる。
この検証は、公開レジストリ版を使うホストの受入やfragment・sticky動作の確認とは別に行う。

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
