## ADDED Requirements

### Requirement: frameと同世代の両軸geometryを公開する

KRRは、利用側がframeとgenerationの一致を検証できる両軸scroll metricsを追加APIで提供しなければならない（MUST）。既存HtmlBrowserFrameのfields/serdeと取得APIを維持しなければならない（MUST）。

#### Scenario: 利用側が表示範囲とバーを判断する
- **WHEN** 利用側が更新frameとmetricsを取得する
- **THEN** generationが一致し、物理寸法/DPRとCSS pixel単位のviewport、content extent、offset、max offset、軸別overflow policyを区別できる
- **THEN** metricsを取得するためにframeが追加で複製・再描画されることはない

#### Scenario: 従来のAPIを利用する
- **WHEN** 既存consumerが従来のframe APIとstruct literalを使う
- **THEN** 既存の公開fieldsとpending updateの意味は維持される

### Requirement: 両軸スクロールを描画と操作へ反映する

KRRは、root documentの横・縦・斜め入力を処理し、描画像、hit-test、DOMのscroll offsetとintersection geometryを同じ座標変換へ反映しなければならない（MUST）。

#### Scenario: DPRの異なる画面で斜めに移動する
- **WHEN** DPR1または2で正負・過大・斜めのphysical scroll deltaを入力する
- **THEN** CSS pixelへ一度だけ変換され、軸ごとに許可rangeへclampされる
- **THEN** 通常contentは両軸で移動し、移動後のpointer targetとobserver geometryが画素と一致する
- **THEN** viewport-fixedはviewportに留まり、stickyとcontaining-blockの既存契約を維持する

### Requirement: root overflowと実際の内容範囲を区別する

KRRは、viewport幅だけでcontent extentを代用してはならない（MUST NOT）。軸別のvisible/auto/scroll/hidden/clipのresolved policyとscroll rangeを区別しなければならない（MUST）。

#### Scenario: 横長の内容と軸別overflowがある
- **WHEN** fixed-width child、unbroken text、clip、root html/bodyのoverflowと軸別指定を組み合わせる
- **THEN** Chromeと同じ入力・資源・状態で可視範囲とroot scroll rangeを検証できる
- **THEN** hidden/clipをuser-scroll可能なauto/scrollと同一視しない
- **THEN** viewport-fixedの内容を誤ってdocumentのscrollable extentへ含めない

### Requirement: resizeと動的更新で状態と世代を整合させる

KRRは、resizeまたはDOM更新後に同一sessionを保ち、両軸のrange/offsetとframeを同じ世代へ更新しなければならない（MUST）。不要なdocument再ロード、資源取得、font構築、重複layoutを避ける再利用条件を検証しなければならない（MUST）。

#### Scenario: スクロールした画面をresizeする
- **WHEN** scroll後にviewportの物理寸法またはDPRを変更する
- **THEN** DOM/JS状態と読み込み済み資源を保持し、必要な再評価だけを行う
- **THEN** 新rangeへのclamp、Y anchor、observer、frame dimensionsとmetrics generationが整合する
- **THEN** 有効resizeの処理失敗時は元errorを返し、閉じたsessionから旧frameを取得できない

### Requirement: 利用側の表示完了を性能条件にする

KRRは、初回実frame、再表示実frame、同一session resizeの時間とRSSを実測し、既存の表示精度・速度・メモリ非劣化の受入条件を維持しなければならない（MUST）。

#### Scenario: 表示精度の修正を受け入れる
- **WHEN** 現行ソースで画像と動的契約の検証を完了する
- **THEN** 既存5phase・5paired runs・各arm5samples（測定条件ごとに両arm合計250 phase records）を維持して時間/RSSを比較し、resizeの測定も追加する
- **THEN** 未計測の改善を性能合格として報告しない
