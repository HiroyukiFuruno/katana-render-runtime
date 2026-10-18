## v0.4.24 の先行リリース条件

HTML #109 は全対象画像90以上・性能非劣化で先行公開する。最終99の改善は #106 で継続し、下記2.2・2.3・2.4・2.6の99/95到達タスクを先行公開の合格として完了扱いしない。連続Yスクロール、全目次、操作・動的script、出典、私有資料のGit除外の検証範囲は維持する。Mermaid #108 の独立117画像各99、完全品質検査、レビュー、CI、公開確認、整理は先行公開でも必須とする。

## 1. 独立基準と私有資料境界

- [x] 1.1 HTML Issueを#106へ統合し、Mermaidを#108へ分離する。
- [x] 1.2 実Chrome PNG/元寸法RMSE・MAE比較と改変画像・寸法不一致の失敗回帰を追加する。
- [ ] 1.3 架空内容fixture、複数幅/scroll、Just/CIを完全検証する。

## 2. HTML精度と操作

- [ ] 2.1 CSS origin、相対文字サイズ、inline style、font行高/baseline、表・flex/gridとauto box sizingを修正し回帰を検証する。
- [ ] 2.2 私有原本ローカル比較と架空fixture全画像で99以上を確認する。
- [ ] 2.3 独立Chromeのsemantic/interaction/performance各95を測定・維持する。
- [ ] 2.4 原本構成を保持した匿名全文のY全範囲を重複付き連続captureし、全16目次/13下位項目の実クリック・実移動先表示・末尾到達を各viewportで検査する。各画像の最小score99を維持する。
- [ ] 2.5 docs/html-fidelity-test-matrix.md / html-supported-feature-inventory.md とfeature catalogで対象機能・通常/境界/操作後caseを固定し、未対応・未実行・未検証を受入成功へ混ぜない。
- [ ] 2.6 HTML/CSSのvisual laneとdynamic scriptのstate/event/task/resource laneを独立検証し、観点別fixtureと全体fixtureの両方を通す。各操作後PNGも独立Chrome基準99以上。

## 3. artifact/session

- [ ] 3.1 provenance、host opt-in/root/quota/atomic保存/clearの公開APIと安全性回帰を検証する。
- [ ] 3.2 独立process hit/miss/更新とfresh session復元後scroll/link/form/JS/closeを検証する。
- [ ] 3.3 JNI/JVMと既存性能・memory・package契約を維持する。

## 4. Mermaidと品質

- [ ] 4.1 node構造の補正、独立公式基準、4枠・members・3継承の構造回帰と英日全99を確認する。構造回帰とAPI契約の合格を独立117画像各99の達成として扱わない。
- [ ] 4.2 Rust/JS直接・推移依存とlockfileを再確認し、完全check/coverage/各scoreゲートを通す。
- [ ] 4.3 v0.4.24包含manifest/release-checkとOpenSpec検証・archiveを完了する。

## 5. 公開まで

- [ ] 5.1 commit/push/Draft PR、最新HEAD review全ページ取得、各指摘修正/reply/resolveを完了する。
- [ ] 5.2 P0/P1 0・未解決0後Ready化、required native CI、expected SHA保護mergeを完了する。
- [ ] 5.3 GitHub Release・assets/runtime/CLIの3crate registry公開と公開版受入を確認する。
- [ ] 5.4 Issue closureとbranch-hygiene、stash0/master clean確認を完了する。

## User Review Phase: ライブラリ委譲を実装へ反映する

- [/] ライブラリ委譲を呼出先だけで判断せず、handle寿命・共有cache・無効化・保持容量まで確認する設計契約をdesign.mdへ固定する。
- [/] 実フォントA→B→A→Bと異なるTTC face indexの再利用を、必須の実フォント・共有metrics同一性・値一致で回帰化する。2件を実行済み。関連候補の61件合格は正式性能・独立画像の受入とは別に扱う。
- [/] 実フォント交互使用のmetrics共有、計算前64MiB制限、正常scope終了、spare側file mutationの回帰を追加して実行する。関連40テスト合格、性能の受入は未完了。
- [/] family大小文字・既存exact名・太字・複数family優先順の実SVG face選択回帰を追加して実行する。関連34テスト合格、図形の受入は未完了。
- [/] 生成JSの同期確認を性能producerのビルド前に必須化する。旧生成物で同期検査失敗、正規再生成後成功、失敗伝播・検査中ソース変更の単体契約2件合格。実CIの受入は未完了。
- [/] 同一呼出の重複proof確認を削減し、入口・出口とrender終了時の検証を維持する。実フォント変更・不存在パス出現・cache/factory回帰35件合格。性能改善率は未測定。
- [ ] 現在ソースで全画像90以上と全対象の速度・memory非劣化を独立確認し、正式ゲートから欠落していないことを検証する。
- [ ] 画像が改善しても性能が劣化した候補を拒否する既存ゲートをCI・release導線で確認し、公開後に#108/#109をcloseする。
