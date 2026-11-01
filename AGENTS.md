# katana-diagram-renderer Agent Rules

## 共有セッション作業台帳の所有者

- subagentは独立したmain sessionではない。`CODEX_THREAD_ID`をmainと共有する場合、`~/.codex/hooks/task-state/<CODEX_THREAD_ID>.json`を作成・上書きしない。workerの完了証跡は本文でmainへ返し、全体台帳はmainだけが更新する。
- mainは委譲時に共有台帳の変更禁止を担当scopeへ含め、subagent完了回収後と終了前に全体タスクID・未完状態の保持を確認する。小taskの完了でリリース全体の未完項目を消してはならない。
- 競合を検知したら、元の観測を保持し、物理タスク台帳から全体を復元する。全項目がdoneのworker台帳を全体完了へ転用しない。検索語: `shared CODEX_THREAD_ID`, `main-owned task ledger`, `worker ledger overwrite`。

## 調査専任エージェントの報告保存

- 読み取り専任のsubagentは調査結果をメッセージ本文だけで返す。報告ファイルの生成を担当に含めず、主担当が必要な記録を`apply_patch`で保存する。実装担当も、報告文をシェル引数へ埋め込んで保存しない。
- アンチパターンは、コマンド例のバッククォートを含む文章をシェルの二重引用符へ入れ、説明中の検査を意図せず実行すること。報告内容と実行するargvを分離し、調査・文書作成から検査を起動しない。
- 誤起動した場合は追加検査を止め、元のtool呼出しと観測出力を保存する。PID・実終了コード・開始時刻・target差分を取得できなかった範囲は未取得と記録し、再実行による置き換えや品質合格への転用をしない。検索語: `report command substitution`, `read-only report text only`, `backtick accidental execution`。

## 非公開の実行観測の表示

- Native CLIの生の`observation.json`は再現用の環境値を含む非公開データである。`cat`、`rg`、辞書全体の`print`やJSON再出力で表示せず、必ず`rtk proxy just native-cli-observation-summary <observation.jsonの絶対パス>`を使う。既存`native_cli_observation_view.py`の固定projectionだけを表示し、schema拒否時に生データ表示へfallbackしない。
- `request.json`やbuild receiptもファイル名・外側の型だけで表示可能と判断しない。許可projection外は表示せず、schema不一致は拒否し、生JSONや部分フィールドへのfallbackをしない。
- 非公開観測を含む`tmp/quality`やownerディレクトリ全体へ内容検索しない。report探索はファイル名の一覧だけで行い、確定した単一reportを許可projectionで読む。複数の`rg --glob`はANDの絞り込みではないため、特定runのglobと`*.json`を併記して探索範囲を広げない。`lifecycle.json`も生環境を含み得るものとして扱う。
- mainは読取り担当へ、必要なreportの正確なパスと許可するフィールドを渡す。集計だけに必要な担当へ生の観測・環境値・credentialsを調べさせず、ログ探索を担当範囲へ暗黙に含めない。環境の復元が必要な主担当も値を内部で使うに留め、ツール出力・Git・公開添付へ出さない。
- 安全な表示要約は表示用のprojectionであり、描画・性能・source一致の受入証明ではない。正式判定は既存producer/validatorで行う。非公開の原本を公開用に書き換え・repinせず、公開添付は別の許可済みpayloadを使う。
- アンチパターンは、入力reportの場所を調べるために観測全体を表示すること。正しくは、既知reportへ直接アクセスし、実行情報が必要な場合だけ安全表示コマンドから参照先を得る。検索語: `private observation environment`, `native-cli-observation-summary`, `safe_summary`, `explicit read scope`。

## 外部依存のfork採用判断

- forkの作成・公開許可と、製品依存への採用判断を分ける。修正パッチの成功だけでforkを既定路線にしない。保守性・要件・性能を満たす既存の代替があれば優先する。
- 採用前に、現行公開版の更新・保守活動、有力な代替の同一KRR入力での描画結果、接続・配布・継続保守の負担を採用評価へ記録する。品質の初期比較に失敗した候補の性能測定は後回しにできるが、未測定を性能劣化の証明にしない。
- 公開APIの不足だけで代替不可と断定せず、既存adapterで補える範囲と上流変更が必要な範囲を区別する。比較未完了は未完了と記録し、公開・本番切替の根拠にしない。
- アンチパターン: patch回帰成功と公開許可からfork採用を確定する。正: 代替比較と維持負担を確認し、適合する低負担の代替がない根拠を記録して限定forkを判断する。検索語: `alternative before fork`, `fork permission is not adoption`, `maintenance cost`。

## 外部依存に由来する検査例外

- 外部ライブラリのAPI形式・型宣言に由来し、KRR側では変更できない検査指摘は、ユーザーへの再確認なしで必要な例外を判断してよい。対象ファイルまたは依存境界、理由、維持する検査範囲を記録し、範囲を限定する。
- 例：外部APIがdefault exportを要求する場合、そのAPIを表す型宣言の該当ルールだけを例外にする。外部の制約を理由に内部実装・テスト全体をLintや厳格な型検査から除外しない。
- 既存の正確な宣言形式で検査が通る場合は、その形式を維持する。外部例外によって描画スコア、カバレッジ、性能の受入基準を下げない。

## Python unitの依存参照

- 厳格な型と公開APIの境界を維持するため、テストのpatch先や依存定数は依存モジュールを直接importして参照する。実装が内部でimportした`subprocess`や別名moduleを、暗黙の公開APIとして経由しない。
- strict mypyのimplicit reexport指摘はテスト側の参照で直し、テスト都合の実装側再export・ignore・型検査緩和で解消しない。仕様として公開した再export自体を検証するケースは区別する。
- この型エラーの修正後は既存`python-typecheck`で先に回収し、既知の失敗を残して重い標準chainを反復しない。機械的検出は既存の必須mypyゲートを維持する。

## Bun Test Scope Gate

- 特定ファイル・ディレクトリの Bun テストは `./` または絶対パスで指定する。裸の相対パスは探索フィルタとなるため使用しない。意図的な負例は隔離した自作fixture内だけで実行する。
- 既存の標準レシピと `--path-ignore-patterns 'tmp/**'` を維持し、実際に実行されたファイル・件数を証跡へ記録する。退避先・過去コピーを含む結果は現行ソースの合格へ転用しない。
- 異常に長い検査は実プロセスの対象パスを確認する。対象逸脱が判明した実行は結果を保存して中断し、対象を修正する。固定待機やタイムアウト延長で放置しない。

## 限定検査の依存範囲

- タスク名だけで「静的のみ」「Cargoなし」と判断しない。実行前にMoonの推移依存と各recipeを確認し、事前buildも含めた実行範囲を資源調整へ渡す。対象逸脱を検知したら、専用process groupの同一性を確認して停止し、実終了・残process・容量差を記録する。
- 現行の`source-static`は`biome`から`rust-facts-host-prebuild`へ到達する。コンパイルを許可しない実行では`source-static-foundation`の依存閉包を確認して使い、`source-static`へ広げない。DAG変更後は分類を再確認する。

## 検査証跡の保存

- 物理identityで束縛済みの証跡・保護ref原本は、容量回復目的でも圧縮・最適化・整形・原子的置換をしない。同じbytes・SHA・modeでもinode変更で証跡は失効する。容量回復は既存cleanによる対象生成物の整理へ限定する。再取得は新runへ保存し、原本差替えやpin更新によって過去の結果を再認定しない。

- 品質判定へ使う検査は、開始前にコマンド・対象ソース・stdout/stderr・実終了コードの保存先を決める。物理ログの取得には `rtk proxy` を使い、RTKの表示用要約を未加工のprocess logとして扱わない。
- 再試行は新しいrunディレクトリへ記録し、command・stdout・stderr・終了コードを上書き拒否で保存する。既存の検査ログを次の実行へ再利用しない。既存の計測実行処理と同じ排他的作成を使い、記録漏れは欠落として残す。
- 保存するargvと実行するargvは同じ配列から渡し、別のシェル文字列へ手作業で転記しない。保護参照も検証済みregistryから同じ配列へ組み立てる。不一致が判明した記録は保持し、実プロセスまたは元のtool呼出しから確認できた実argvと、その回で保証できた範囲を別途記録する。
- 保護refと容量stateの事前検証は `just protected-capacity-admission <registry>` の既存validatorを使い、返された `environmentValue` を子プロセスへ渡す。registry schema、`size`、完全な `st_mode`、物理stampの検査を一時コードへ複製しない。この出力をbuildやcleanupの合格として扱わない。
- 診断の再実行には検査本体の実行観測に保存した環境を内部で使う。admission後に追加する `CARGO_CAPACITY_PROTECTED_REFS` もhostの環境fingerprint対象であり、source-beforeの環境と同一とはみなさない。環境全体を表示・Git登録・公開せず、差分の項目名とdigestだけを記録する。
- ソース集合は既存のnative source manifestを優先し、ソース内容digest・manifestファイルSHA・独自serializationのSHAを区別して記録する。異なる算出方式の値を同じdigestとして照合・報告しない。
- 取得済みの結果が要約だけなら、観測したtool出力と終了コードを保全し、未取得の範囲を明記する。欠けた個別結果を再構成しない。ソースが変わった場合は、再検証した範囲と変更前の成功を区別する。

## 限定Rust検証の実行条件

- Rust実装・テストを追加または分割した後は、既存`ast-lint`の成功を、限定catalog取得やruntimeの再コンパイルより先に回収する。`source-static-foundation`やClippyの成功だけでは、AST側のファイル行数・責務境界検査を通したことにならない。新test helperで既存fileの300行境界を超え、実テスト成功後のcommitで初めて拒否される反復を避ける。閾値を緩めず、観点単位で分離して同じ検査を再実行する。

- Rustup管理下のCargoは既存Justfileのlauncher経由で起動する。証跡の実体SHA取得に解決した`cargo.executable`を実行argvへ置き換えない。実体を直呼びすると子rustcのRustup選択が依存crate内のtoolchain指定へ変わり、同一targetで異なるcompilerが混在する。E0514時は実argv・依存側toolchain指定を照合し、標準launcherで再実行する。容量やcache破損と断定して不要なcleanをしない。
- Cargoを伴わない予備の直接rustfmt検査も、対象manifestのeditionを明示する（現workspaceは`--edition 2024`）。既定editionによる整形成功を`cargo fmt`と同条件の成功に扱わず、不要な既存整形差分を混入させない。正式判定では既存`fmt-check`を維持する。
- 絞り込み検証も既存recipeを優先する。引数を受けるrecipeがない場合は `just --command cargo ...` を使い、Justfileの作業場所・dotenv・export済みの `RUSTFLAGS` 等を保持する。既存ゲートと同じworkspace・features・targetsを保ち、libtestのcase filterで絞る。変更が必要なら別fingerprintとなる範囲を明記し、別条件の成功を元ゲートへ転用しない。別条件の直接Cargo実行で依存物を再コンパイルしない。既存binaryの診断実行は、同じbinaryの物理SHAと対象caseを記録し、ソース変更後の合格へ転用しない。
- Rustのmodule名とcase名をファイル名から推測しない。importは実際の`mod`宣言と可視性へ照合し、限定caseの完全名は同条件のコンパイル済みbinaryの`--list`等、既存runnerの実catalogへ照合する。catalogのbinary SHAとrawを保存し、選択した各caseが意図した件数だけ実行されたことを確認する。0件や未選択のcaseを成功に含めない。
- 結果の件数判定をsummary文の単数形・複数形や色付けへの完全一致へ依存させない。既存runnerの構造化結果があれば優先し、実終了コード・個別case結果・実行件数を照合する。後処理の失敗と検査本体の失敗を区別し、原失敗を保持して実rawから訂正する。訂正で未実行caseを補完しない。

## 容量不足時の回復手順

- 容量を理由に検査が止まった場合は、実ディスクの空き容量と、repoの固定容量閾値・対象生成物の使用量を分けて記録する。容量不足という説明だけで停止しない。
- 独自の調査や容量ガードの変更より先に、既存の `just clean` で回復できるか判断する。使用中のbuild・登録worktree・保護対象を確認し、必要な証跡を保持して実行する。実ディスクの空きも不足する場合は、明示承認済みの範囲で上位privateの `just clean-all` を使う。閾値の引き上げや基準のリセットで通過させない。
- clean後は実ディスクと対象生成物を再測定し、本来の検証へ戻る。削除されたbinaryに依存する証跡は過去のものとして保持し、必要なbuild・registry・receiptを正式producerで新規生成する。古い証跡を書き換えて再利用しない。
- 上位のcleanが対象にしない入れ子のworktreeも容量調査では確認するが、大きさだけを理由に一括削除しない。定型的な回復で解消する問題に、追加の容量基盤や診断の反復を持ち込まない。
- 共有workspaceのclean前は、各ownerと実行中buildだけでなく、予約済み検証が再利用するbinaryの保持も照合する。再生成可能であることだけを理由に予約済みのwarm成果物を削除しない。共有clean後は実path/SHAを再確認し、消失していればwarm前提の時間・容量枠で起動せず、cold条件へ調整する。全体exitが非0でも部分削除は起こり得る。

## 基準・候補の証跡を混同しない

- 未改変のライブラリ経路はpackage版だけで認定しない。実際のSDK差込環境・リンク先・binary-cacheの選択も確認し、保持SDKを公開配布物による対照として扱わない。
- 基準データの診断はディレクトリ名だけで対象を判断しない。既存の厳格readerを使い、実入力パス、receiptのschemaとrole、fixture inventoryを確認してから結論を出す。official rasterとcandidate nativeの証跡は別契約として扱う。
- candidate nativeの`sourceCurrentPins`不一致をofficial raw JS基準の無効理由へ転用しない。基準の再利用可否はその基準を束縛するreceipt・process・tool・資源・設定の実証跡で判定し、reader未実行の推論と実際の拒否を区別する。
- 誤った調査は元証跡を保持して訂正する。新しいSHAへの差し替えや過去画像の再認定で不一致を隠さない。

## HTML Consumer Acceptance Gate

- KRRの目的は、利用側が描画結果を正しく表示・操作できること。画像の一致だけで完了とせず、フレームの寸法・倍率・両軸のコンテンツ範囲と位置、入力座標、resize後の更新が利用側へ届く契約を確認する。UIのバー描画は利用側の責務として区別する。
- 表示までの時間を性能評価の第一優先とし、初回表示・再表示・同一セッションのresizeを区別して実測する。メモリ非劣化も維持し、計測していない改善を解消済みとしない。
- resize・スクロール・動的更新では、DOM・JavaScript状態・読み込み済み資源・フォント等の再利用と無効化条件を実装で追う。セッションを保持しているだけでキャッシュが十分と判断しない。不要な再parse、資源取得、フォント構築、重複layoutを確認する。
- アンチパターンは「画像スコアが合格なので利用側も表示可能」「resize関数があるので高速」。正しくは、利用側との契約回帰、状態維持・無効化の回帰、実フレームが得られるまでの時間を対応付けて確認する。検索語: `consumer acceptance`, `scroll extent`, `resize cache`, `time to frame`。

## 性能履歴と測定結果の再利用

- 性能測定の履歴と比較に使う現行基準は`docs/performance/html-performance-history.json`でGit管理する。`records`へ測定履歴を追加し、`activeReferenceRecordId`で現行基準を明示する。初期基準は最初の合格済み測定から選ぶ。回答・再計測の前に履歴を確認し、測定元commit、入力条件、試行数、時間・RSS、判定と証跡digestを記録する。会話やtmpだけを性能結果の保存先にしない。
- 性能改善を同一の正式条件で実測し、全対象の受入条件を満たしたときは、新しいrecordを追加して現行基準をそのrecordへ更新する。過去のrecordと測定値は変更せず、Git履歴で追跡する。同等結果、測定揺らぎ、一部改善と引換えの別phase時間またはメモリ悪化では現行基準を更新しない。候補84条件すべての表示3秒未満と、その時点で比較可能な全条件の速度・メモリ非劣化を維持する。
- 現行基準の自動昇格と正式性能gateへの接続は未実装であり、`activeReferenceRecordId`の選択だけで改善判定・合格を意味しない。
- 元の測定結果と現行版への再利用適格性を分ける。同じ実行ファイル・入力/資源/フォント/操作・計測プログラム/ツール/環境・判定条件なら、既存の厳格なvalidatorで原結果を再検証し、測定を重複実行しない。文書やtestだけの変更、Git HEADの変化だけを全再計測の理由にしない。
- HEAD結合などで有効な結果を再利用できない場合は、既存検証の等価性契約を直す対象として追跡する。元receiptのhead/pinを書換えたり、Gitの集計値だけで現行版を合格にしたりしない。製品や測定条件が変わった場合は影響範囲を確認して再計測する。
- 再利用を予約したbinary・closed原結果は既存の保護対象へ含める。cleanで失効した証跡を新binaryへ付け替えない。私有資料・生環境・認証情報はGitへ入れず、公開用の固定projectionだけを履歴へ保存する。
- アンチパターン: 過去測定を探さず毎回全84条件を測り直す。正: Git履歴を確認→既存原結果の適格性を検証→変更が影響する条件だけを必要な範囲で再計測。検索語: `performance history`, `reuse before remeasure`, `same binary`, `historical evidence`。

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

- 再利用keyと本体のidentityは、使用版ライブラリの実注入環境で照合する。採取側にはないretry等の実行管理metadataは、公式実装と固定検査の消費経路を確認して限定除外し、未知名はbindする。根拠は値や単変数hashではなく、キー名と集合digestの同一性で保存する。

- 同じrepo・検査範囲の実行所有者は1名に固定する。toolが`session_id`または実行中cellを返したら、そのIDで終了コードを回収する。終了ファイルがまだ無いことを再起動理由にしない。再試行は旧プロセスの終了・実終了コード・ログ保全を確認してから新runへ進む。
- 標準品質ゲートは既存のterminal-report排他lockを使う。限定検証も同じ既存`local_review_lock.review_lock`で所有権を確保し、取得できなければ既存ownerの結果を回収する。独立して並列化できる検査は、所有者が既存DAG内で並列実行する。重複起動時は各runの実PID・argv・中断結果を保持し、current sourceの成功へ転用しない。
- 排他lockの寿命は実際の検査・更新・その子processの終了回収までとする。永続REPL、会話、調査や次のコマンドの準備中まで保持しない。各実行のcontext managerで取得・解放し、例外時も解放する。自分で排他を取得する標準producerには親から同じlockを重ねない。アンチパターン: idleのREPLがlockを持ち続け、本人の`just depends-update-all`を妨げる。正: 実行中だけ排他し、終了後は別processによる取得・解放も確認する。競合時もlock fileの削除や排他回避で通さない。検索語: `idle lock retention`, `command scoped lock`, `depends-update-all lock`。

- main agent は司令塔として、設計、ハーネス、担当分離、統合レビュー、最終ゲートを担う。分離可能なreview指摘修正をmainが直列実装しない。
- 変更ファイルと責務を先に棚卸しし、同時実行枠の範囲で1ファイルまたは非重複責務ごとにsubagentへ並列委譲する。空き枠を放置して直列化しない。
- subagent起動前に、最新のユーザー指示と利用可能モデルを確認する。利用不可のモデルを選ばず、限定実装はLuna、複雑な設計・統合分析はTerraへ切り替える。
- main agent はsubagent結果を鵜呑みにせず、追加fixture、差分レビュー、完全ゲートで統合判定する。
- 完了済みsubagentへの追加作業は `followup_task` で起動し、稼働状態を確認する。`send_message` は通知だけであり、追加作業を開始した証拠にしない。
- 実行・追加調査・修正を求める依頼は、対象の直前状態にかかわらず `followup_task` を使う。依頼後は `list_agents` で稼働を確認し、着手宣言だけのfinalを実装完了として受け入れない。完了判定には担当scope内の実diff・検査結果などの物理成果を照合する。
