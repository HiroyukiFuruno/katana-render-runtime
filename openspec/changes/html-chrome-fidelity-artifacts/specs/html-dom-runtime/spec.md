## MODIFIED Requirements

### Requirement: KRR は browser-equivalent HTML page を所有しなければならない

システムは、KRR 内の in-process Rust/V8 persistent session で HTML parsing、CSS cascade/layout/paint、JavaScript event semantics、input、navigation を評価しなければならない（MUST）。KRR は KatanA から受け取った raw HTML を doctype、comment、head/body 構造を壊す host-side 文字列変換で書き換えてはならない（MUST NOT）。interactive runtime は既存の Rust/V8 DOM/CSS primitives を source of truth として強化し、static HTML export/render path を interactive viewer に転用してはならない（MUST NOT）。Chromium、WebView、外部 helper process、browser binary は 製品runtime、runtime unit test、release artifact に導入してはならない（MUST NOT）。session を開始できない場合、KDV/KatanA は typed error を表示し、static export image へ fallback してはならない（MUST NOT）。

#### Scenario: JavaScript が画面を動的に変更する

- **WHEN** local HTML の script または user action が DOM、style、form control、timer を変更する
- **THEN** KRR は browser engine が repaint した最新 frame を返す
- **THEN** KDV または KatanA は DOM/CSS/JavaScript を解釈せずに最新 frame を表示する

#### Scenario: 一つの script が実行時例外を送出する

- **WHEN** interactive document の一つの script が通常の JavaScript 実行時例外を送出する
- **THEN** KRR は document URL と例外位置を structured runtime log に記録する
- **THEN** KRR は後続 script、lifecycle event、layout、frame 出力を継続する
- **THEN** JavaScript 構文エラー、DOM bridge failure、execution timeout は session 起動失敗として返す

#### Scenario: head metadata と executable content を HTML semantics で評価する

- **WHEN** HTML が `title`、`style`、`script` と interactive body content を含む
- **THEN** browser engine は head metadata、stylesheet、script を HTML semantics に従って評価する
- **THEN** KatanA surface は `title`、stylesheet source、script source を本文 text として表示しない
- **THEN** stylesheet と script の効果は browser frame と action 後の frame update に反映される

#### Scenario: raw HTML の parser mode を保持する

- **WHEN** local file の raw HTML が doctype を持たない、または comment 内に `<head>` text を含む
- **THEN** KRR は完全な document URL origin と raw HTML を in-process runtime に渡す
- **THEN** host は doctype、`<base>`、navigation script を HTML text に挿入せず、KRR parser/runtime が document tree を決定する


#### Scenario: 実Google Chromeによる独立参照検証

- **WHEN** ユーザー指定の画像比較ゲートがChrome参照を取得する
- **THEN** 検証専用script/CIは実Google Chromeを使用できるが、KRR runtime・crate配布・host製品依存には含めない

### Requirement: KRR in-process runtime は外部 browser asset なしに検証可能でなければならない

システムは、KRR crate に含まれる Rust/V8 runtime だけで interactive session を起動できなければならない（MUST）。release workflow は dependency graph、package contents、runtime configuration を検証し、Chromium binary/archive/manifest、browser download、external helper、`KRR_CHROME_BIN` のような browser override が含まれないことを確認しなければならない（MUST）。

#### Scenario: KatanA release artifact が外部 browser runtime を要求しない

- **WHEN** KatanA release workflow が公開済み KRR version を解決する
- **THEN** workflow は crates.io package と application dependency だけを検証し、browser bundle を取得または executable 隣接へ配置しない
- **THEN** `HtmlRuntime::open` は開発機の Chrome、Keychain、Cargo build directory、external helper に依存しない

#### Scenario: forbidden external browser asset が混入している

- **WHEN** 製品package、製品packaging workflow、runtime configuration が Chromium/WebView/browser download/helper を参照する
- **THEN** release workflow は publish または KatanA packaging より前に失敗する
- **THEN** workflow は external browser asset を追加して問題を回避しない


#### Scenario: 実Google Chromeによる独立参照検証

- **WHEN** ユーザー指定の画像比較ゲートがChrome参照を取得する
- **THEN** 検証専用script/CIは実Google Chromeを使用できるが、KRR runtime・crate配布・host製品依存には含めない

## ADDED Requirements

### Requirement: HTMLは独立Chrome画像99と操作互換性を維持する

KRR MUST compare independently rendered PNGs with actual Google Chrome at the same viewport/scale/scroll/font/resource state. 各画像はRMSE/MAEの最小score99以上を満たさなければならない。自己比較、crop/resize、閾値緩和を禁止する。元semantic/interaction/performance各95以上と既存完全品質・coverage基準を維持する。私有資料はローカル読取・ignored出力だけに限定し、似た構成の架空内容fixtureをGitへ採用する。

#### Scenario: 幅とscrollの異なる画面を検証する
- **WHEN** 初期表示、複数viewport、複数scroll位置のKRRとChromeを撮影する
- **THEN** 各独立PNG・diff・scoresを保存し、寸法不一致や一つでも99未達ならゲートが失敗する

#### Scenario: リンク・フォーム・JS・scrollと性能を比較する
- **WHEN** 同じ操作列をChromeとKRRへ入力する
- **THEN** 独立したsemantic/interaction/performance判定を各95以上で検証し、公開registry版でも結果を確認する

### Requirement: HTML timerの時間進行はhost明示契約である

KRR MUST expose additive host control for advancing the deterministic HTML timer clock. The host MUST pass elapsed time explicitly; KRR MUST NOT start a background wall-clock thread or silently execute non-zero-delay timers during an ordinary zero-elapsed lifecycle pump. A timer pump that executes callbacks MUST publish the resulting frame, while a pump that reaches no deadline MUST preserve the current frame generation.

#### Scenario: hostがtimer deadlineを進める

- **WHEN** hostが`HtmlBrowserSession::advance_time(Duration)`へdeadlineまでの経過時間を渡す
- **THEN** deadline未到達ならcallbackとframe generationは変化せず、到達時だけcallbackとframe updateが発生する
- **THEN** session navigationはtimer clockとpending callbackを新documentへ持ち越さず、closed sessionはtyped errorを返す
