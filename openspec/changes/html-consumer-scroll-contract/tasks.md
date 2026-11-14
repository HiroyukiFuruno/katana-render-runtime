一次台帳は`docs/quality-gate-improvement-tasks.md`のR03/R04。この一覧は同項目の実装分解であり、リリース全体の合格条件を置き換えない。

## 1. 座標とoverflow

- [ ] 1.1 公式CSS Overflow仕様と同条件Chromeを参照し、root html/body伝播、軸間computed value、hidden/clipの挙動を既存fixtureへ回帰として固定する。
- [ ] 1.2 軸別overflow方針をstyle解決へ保持し、既存clip/layoutの挙動を壊さずroot policyを取得する。
- [ ] 1.3 既存layoutのbox/line/clip/positioning ownershipから両軸scrollable extentを集計し、長文・横長要素・fixed・transform・clipを検証する。

## 2. 両軸の実処理

- [ ] 2.1 同じCSS pixel offsetをX/Yのsession stateとpaint/clip/media/positioningへ接続する。
- [ ] 2.2 斜め入力を1更新としてclampし、hit-test、DOM scroll offset、bounding rectとobserverへ同じ変換を接続する。
- [ ] 2.3 DPR1/2、正負/過大入力、fixed/sticky/containing-blockとpointer targetの画素・実入力回帰を実行する。

## 3. 公開契約とresize

- [ ] 3.1 既存frameのfields/serde/APIを維持し、frameと同世代のmetrics取得APIとpending消費規則を実装・検証する。
- [ ] 3.2 resize/DOM更新でextentとoffsetを更新し、Y anchor、observer、pixel dimensions、error/close契約を検証する。
- [ ] 3.3 DOM/JS/資源・fontの再利用とmeasurement cache無効化条件を回帰で検証し、不要な完全layoutを追加しない。

## 4. 統合検証

- [ ] 4.1 現行ソースの静的解析・重複検査・関連native回帰を既存ゲートで実行し、失敗を解消する。
- [ ] 4.2 既存5phaseの時間/RSSと同一session resizeを実測し、非劣化を確認する。
- [ ] 4.3 KRR契約と利用側接続の未対応を区別して一次台帳へ証跡を戻し、全体の画像/動的/coverage/review/CI/公開条件と統合する。
