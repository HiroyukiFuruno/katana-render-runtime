# root coverageの安全な再試行

作業の完了状態は`docs/quality-gate-improvement-tasks.md`のQ19/Q20で管理する。本書はKRR固有のroot producerの設計契約であり、汎用runnerや新しいcache storeを定義しない。

## 利用する既存機能

Nextest 0.9.148のrecord/rerunが合格済みcaseの選択を担当する。再試行には完全なrun UUIDを渡し、`latest`やprefixを使用しない。cargo-llvm-covの`--no-report`と最後の`report`で計測を集約する。`--no-report`自体が自動cleanを抑止するため、併用が拒否される`--no-clean`は渡さない。設定・依存・ツール・環境・sourceの同一性は既存のlocal coverage identity、改変検知は既存のGit-dir private anchorを使う。

公式仕様: [Nextest rerun](https://nexte.st/docs/features/record-replay-rerun/rerun/)、[record store](https://nexte.st/docs/features/record-replay-rerun/managing-runs/)、[cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov#merge-coverages-generated-under-different-test-conditions)。Nextest自身はsource変更を追跡しないため、KRRの同一性検査を省略しない。

## 固定する範囲

- root workspace・all targets・all features・locked・krr-coverage・既存Nextest設定・retries 0を維持する。任意filterは受け付けない。
- 対象はroot producerのみ。baseline/candidateの既存独立producerを変更しない。
- root producerの開始、終了、100%・未計測0、毎回のaggregate検査は維持する。
- `CARGO`と`TEST_THREADS`をJustの実値でproducerへ渡し、記録と実行を同じ値にする。

## 再利用の条件

同一のroot identity、完全UUID、実Nextest recordとindex、実テストbinary群、保持したprofrawが必要である。個別形式を再実装せず、既存の安定したtree digestで`target/nextest-root-state`と`target/llvm-cov-target`全体を束縛する。これによりrecord内のinventory、index、binary、依存artifact、profileの欠落・変更を検知する。symlink・不安定な読出し・未対応host・不明な構成は再利用しない。

checkpointは固定producer `root-nextest-retry`として既存private anchorへ束縛する。checkpointの有効性はテスト・coverage合格を意味しない。保存できる実test exitは0または100だけとし、実record UUIDを特定できない場合は次回full実行へ戻る。checkpointはcacheとして更新するが、各attemptのargv・stdout・stderr・実exitは新規directoryへ排他的に保存して上書きしない。

## 実行順序

1. 既存local beginで現在のidentityを確定し、root専用の既存lock primitiveで同時producerを拒否する。
2. 認証済みcheckpointと現在のartifact treeが一致すれば再利用する。不一致・破損・欠落はfull実行へ戻る。
3. fullの場合だけ既存guard経由で`clean --workspace --profraw-only`を実行し、過去のraw計測値を除いてから全対象を`--no-report`で実行する。コンパイル済みprofileは容量整理から保護し、Cargo自身のfingerprintで必要な再コンパイルを判断する。再試行の場合はcleanせず、固定した完全UUIDを`-R`へ渡す。attemptごとに別のLLVM profile名を使用する。未対応host等で既存contextがcache不可と判定した場合は、artifact用のhost依存APIを呼ばず、全test・reportを実行する。
4. subprocessへ渡したargvと保存argvは同じ配列にする。未承認filter、別profile、別target、任意の追加引数は既存capacity validatorで拒否する。
5. test exit 100なら実record・treeをcheckpointへ束縛して失敗を返す。compile等の他の失敗を成功や再利用可能なtest結果に変換しない。
6. test exit 0、または同一性を満たす過去exit 0なら、保持したprofileで正式JSON reportを生成する。後者はテストを再実行しない。report失敗時も元の成功済みtestの束縛を保てる場合だけ再利用する。
7. 既存local collect finishとaggregateが真正なreportと100/0を検査する。source/contextが途中で変わった場合は受理しない。

## 検証

R247の隔離した実ライブラリ検査では、profile-only cleanでraw計測値が全削除され、profdataとコンパイル成果物のSHAが保持された。clean後に別のcaseだけを実行したreportでは、以前のcase専用関数のcountが1から0となり、report反復のJSON dataも一致した。これは本体の時間・容量効果や100/0を保証しない。既存の固定容量上限による別の整理では成果物が失効し得るため、毎回のsnapshot照合を維持する。

既存の必須Lint・strict型検査・重複検査・owned source/recipe契約に含める。実ファイルを用いたartifact欠損・改変・symlink・identity変更でfullへ戻るunit契約を追加する。既存Nextestとcargo-llvm-covを実行する隔離した自作fixtureで、合格caseの実行回数不変、失敗caseの再実行、保持profileからの実reportを確認する。fixtureの成功をKRRの100/0へ代用しない。KRR現行sourceの完全ゲート、性能、画像、3OS、公開条件は別途維持する。
