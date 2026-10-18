# katana-diagram-renderer Agent Rules

## 外部依存に由来する検査例外

- 外部ライブラリのAPI形式・型宣言に由来し、KRR側では変更できない検査指摘は、ユーザーへの再確認なしで必要な例外を判断してよい。対象ファイルまたは依存境界、理由、維持する検査範囲を記録し、範囲を限定する。
- 例：外部APIがdefault exportを要求する場合、そのAPIを表す型宣言の該当ルールだけを例外にする。外部の制約を理由に内部実装・テスト全体をLintや厳格な型検査から除外しない。
- 既存の正確な宣言形式で検査が通る場合は、その形式を維持する。外部例外によって描画スコア、カバレッジ、性能の受入基準を下げない。

## Bun Test Scope Gate

- 特定ファイル・ディレクトリの Bun テストは `./` または絶対パスで指定する。裸の相対パスは探索フィルタとなるため使用しない。意図的な負例は隔離した自作fixture内だけで実行する。
- 既存の標準レシピと `--path-ignore-patterns 'tmp/**'` を維持し、実際に実行されたファイル・件数を証跡へ記録する。退避先・過去コピーを含む結果は現行ソースの合格へ転用しない。
- 異常に長い検査は実プロセスの対象パスを確認する。対象逸脱が判明した実行は結果を保存して中断し、対象を修正する。固定待機やタイムアウト延長で放置しない。

## 検査証跡の保存

- 品質判定へ使う検査は、開始前にコマンド・対象ソース・stdout/stderr・実終了コードの保存先を決める。物理ログの取得には `rtk proxy` を使い、RTKの表示用要約を未加工のprocess logとして扱わない。
- 再試行は新しいrunディレクトリへ記録し、command・stdout・stderr・終了コードを上書き拒否で保存する。既存の検査ログを次の実行へ再利用しない。既存の計測実行処理と同じ排他的作成を使い、記録漏れは欠落として残す。
- 保存するargvと実行するargvは同じ配列から渡し、別のシェル文字列へ手作業で転記しない。保護参照も検証済みregistryから同じ配列へ組み立てる。不一致が判明した記録は保持し、実プロセスまたは元のtool呼出しから確認できた実argvと、その回で保証できた範囲を別途記録する。
- 保護refと容量stateの事前検証は `just protected-capacity-admission <registry>` の既存validatorを使い、返された `environmentValue` を子プロセスへ渡す。registry schema、`size`、完全な `st_mode`、物理stampの検査を一時コードへ複製しない。この出力をbuildやcleanupの合格として扱わない。
- ソース集合は既存のnative source manifestを優先し、ソース内容digest・manifestファイルSHA・独自serializationのSHAを区別して記録する。異なる算出方式の値を同じdigestとして照合・報告しない。
- 取得済みの結果が要約だけなら、観測したtool出力と終了コードを保全し、未取得の範囲を明記する。欠けた個別結果を再構成しない。ソースが変わった場合は、再検証した範囲と変更前の成功を区別する。

## 限定Rust検証の実行条件

- 絞り込み検証も既存recipeを優先する。引数を受けるrecipeがない場合は `just --command cargo ...` を使い、Justfileの作業場所・dotenv・export済みの `RUSTFLAGS` 等を保持する。既存ゲートと同じworkspace・features・targetsを保ち、libtestのcase filterで絞る。変更が必要なら別fingerprintとなる範囲を明記し、別条件の成功を元ゲートへ転用しない。別条件の直接Cargo実行で依存物を再コンパイルしない。既存binaryの診断実行は、同じbinaryの物理SHAと対象caseを記録し、ソース変更後の合格へ転用しない。

## 容量不足時の回復手順

- 容量を理由に検査が止まった場合は、実ディスクの空き容量と、repoの固定容量閾値・対象生成物の使用量を分けて記録する。容量不足という説明だけで停止しない。
- 独自の調査や容量ガードの変更より先に、既存の `just clean` で回復できるか判断する。使用中のbuild・登録worktree・保護対象を確認し、必要な証跡を保持して実行する。実ディスクの空きも不足する場合は、明示承認済みの範囲で上位privateの `just clean-all` を使う。閾値の引き上げや基準のリセットで通過させない。
- clean後は実ディスクと対象生成物を再測定し、本来の検証へ戻る。削除されたbinaryに依存する証跡は過去のものとして保持し、必要なbuild・registry・receiptを正式producerで新規生成する。古い証跡を書き換えて再利用しない。
- 上位のcleanが対象にしない入れ子のworktreeも容量調査では確認するが、大きさだけを理由に一括削除しない。定型的な回復で解消する問題に、追加の容量基盤や診断の反復を持ち込まない。

## 基準・候補の証跡を混同しない

- 基準データの診断はディレクトリ名だけで対象を判断しない。既存の厳格readerを使い、実入力パス、receiptのschemaとrole、fixture inventoryを確認してから結論を出す。official rasterとcandidate nativeの証跡は別契約として扱う。
- candidate nativeの`sourceCurrentPins`不一致をofficial raw JS基準の無効理由へ転用しない。基準の再利用可否はその基準を束縛するreceipt・process・tool・資源・設定の実証跡で判定し、reader未実行の推論と実際の拒否を区別する。
- 誤った調査は元証跡を保持して訂正する。新しいSHAへの差し替えや過去画像の再認定で不一致を隠さない。

## HTML Consumer Acceptance Gate

- KRRの目的は、利用側が描画結果を正しく表示・操作できること。画像の一致だけで完了とせず、フレームの寸法・倍率・両軸のコンテンツ範囲と位置、入力座標、resize後の更新が利用側へ届く契約を確認する。UIのバー描画は利用側の責務として区別する。
- 表示までの時間を性能評価の第一優先とし、初回表示・再表示・同一セッションのresizeを区別して実測する。メモリ非劣化も維持し、計測していない改善を解消済みとしない。
- resize・スクロール・動的更新では、DOM・JavaScript状態・読み込み済み資源・フォント等の再利用と無効化条件を実装で追う。セッションを保持しているだけでキャッシュが十分と判断しない。不要な再parse、資源取得、フォント構築、重複layoutを確認する。
- アンチパターンは「画像スコアが合格なので利用側も表示可能」「resize関数があるので高速」。正しくは、利用側との契約回帰、状態維持・無効化の回帰、実フレームが得られるまでの時間を対応付けて確認する。検索語: `consumer acceptance`, `scroll extent`, `resize cache`, `time to frame`。

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
- 初回AIレビューは Draft の `@codex review` で行う。Draft作成前と指摘修正時は自己レビュー・必要な静的検査・関連回帰を行い、ローカルHigh reviewを重複実行しない。必要時だけ `just local-review` を明示する。`just check` と pre-push は標準品質laneを維持する。
- Draftではremoteの完全CIを実行せず、`ready_for_review` とReady後のpushで必須CIを実行する。DraftのSKIPを品質検証成功として再利用しない。
- 動作保証のCI対象はLinux x64、Windows x64、macOS Apple Silicon。Intel Macは保証対象外とし、必須matrixとrequired status checkへ戻さない。
- ローカルmacOS検査証跡は任意。有効な同一PR/base/HEAD・全macOS検査範囲の保守者証跡があれば再利用し、未提出・不一致・失効・取得失敗ならApple SiliconのCIで実検査する。証跡提出をReadyやリリースの前提にしない。
- Draft で最新 HEAD に対する `@codex review` と自己レビューを実施し、GitHub APIで Issue コメント、formal reviews、review threads を全ページ取得する。初回と再レビュー2回を通常目安とする。
- 各指摘を個別に分類し、P0/P1は必ず修正、P2/P3は要件・互換性・DoDへの影響を根拠付きで判断する。不要な改善は理由を記録し後続patchへ送れる。修正後は自己レビュー、関連する静的検査と回帰確認を行い、push、該当threadへのreply、threadのresolveを個別に完了する。静的解析だけで実動作を保証したとは扱わない。
- レビュー回数を理由に未解決P0/P1、DoD違反、最新HEADのPR review、個別reply/resolve、required native checks、expected SHA付きprotected mergeを省略しない。
- 最新 HEAD のレビュー結果、未解決thread 0、Issue/DoDを確認してからReady化し、Readyで起動したrequired CI checksの成功を確認する。その後は保護されたPR mergeを、merge直前に再取得したexpected HEAD SHAで実行し、人間による直接master更新やadmin bypassを使わない。

## Orchestration Gate

- main agent は司令塔として、設計、ハーネス、担当分離、統合レビュー、最終ゲートを担う。分離可能なreview指摘修正をmainが直列実装しない。
- 変更ファイルと責務を先に棚卸しし、同時実行枠の範囲で1ファイルまたは非重複責務ごとにsubagentへ並列委譲する。空き枠を放置して直列化しない。
- subagent起動前に、最新のユーザー指示と利用可能モデルを確認する。利用不可のモデルを選ばず、限定実装はLuna、複雑な設計・統合分析はTerraへ切り替える。
- main agent はsubagent結果を鵜呑みにせず、追加fixture、差分レビュー、完全ゲートで統合判定する。
- 完了済みsubagentへの追加作業は `followup_task` で起動し、稼働状態を確認する。`send_message` は通知だけであり、追加作業を開始した証拠にしない。
