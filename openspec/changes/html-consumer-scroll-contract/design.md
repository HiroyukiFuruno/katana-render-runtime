## Context

既存frameのviewportとpixel bufferは物理pixel、scroll_y/content_heightはCSS pixelである。public structへのfield追加はdownstreamのstruct literalを壊す。X deltaは現在破棄され、overflowのhidden/clip/auto/scrollとX/Y指定も単一Clipへ集約される。描画、hit-test、fixed/sticky、observerは同じ座標規約で更新する必要がある。

## Goals / Non-Goals

**Goals:** root documentの両軸geometryを同じframe generationと組にして公開し、実際の描画・操作と一致させる。既存API、DOM/JS状態、資源再利用と性能条件を守る。

**Non-Goals:** この変更でUIのスクロールバーを描画しない。nested scroll containerは別の機能として扱うが、既存clip/containing-block/observerを破壊してよいという意味ではない。別repoの編集を無許可で行わない。

## Decisions

### 世代を共有する追加API

`HtmlBrowserFrame`のfields/serde/既存取得APIを維持し、frameと同世代の`HtmlBrowserScrollMetrics`を取得する追加APIを提供する。metricsは論理viewport、content extent、offset、max offset、軸別overflow policyを持つ。単独の最新metrics参照を古いframeと組み合わせる設計は避け、取得とpending消費の意味を明示する。永続frame artifact形式へ暗黙に新metadataを追加しない。

### 既存layoutと座標系を共有する

既存layout/paintのbox、text/replaced content、clip、positioning ownershipからrootのscrollable extentを導く。単にviewport幅や全boxの最大xを使うと、長文・clip・viewport-fixedで誤る。描画位置、hit-test、DOMのscrollX/pageXOffset、getBoundingClientRectとintersection geometryを同じCSS pixel offsetで扱う。入力のphysical deltaはDPRで一度だけ変換し、斜め入力は1回の更新として処理する。

fixedはviewport基準を維持し、transformがfixed containing blockを作る場合は既存ownershipに従う。SVG全体を平行移動する実装は採用しない。stickyは既存Y動作を維持しながらXのdocument移動と整合させる。

### overflowとresize

X/Yのresolved policyと、実際のrange/user-scrollabilityを分離する。hiddenのprogrammatic scrollとclipの非scrollable性を混同しない。root html/bodyの伝播と軸間computed-value規則は公式CSS仕様と同条件Chromeで確認し、現在のClip aliasへ戻さない。

resizeでは同一sessionのDOM、JS、読み込み済み資源を維持する。変更されたviewport/style/内容に応じて再layoutとmeasurement cacheの無効化を行い、offsetをclampし、Y anchor/observer/metrics/pixelを同じ世代でcommitする。失敗時は既存のclose-and-original-error契約を守る。

### 検証と性能

既存native/Chrome基盤でDPR1/2、横長boxとunbroken text、正負/斜め/過大入力、fixed/sticky/clip、resizeとDOM更新を検証する。静的解析と関連回帰を先行し、画像と実入力を含む全体検証へ進む。既存の5phase・5paired runs・各arm5samples（測定条件ごとに両arm合計250 phase records）を維持し、同一session resizeは追加で実測する。メモリと実frameまでの時間の非劣化は設計上の期待で代用しない。

## Risks / Trade-offs

- extent集計の追加cost → 既存layout走査へ統合し、別の完全layoutやhot pathでのasset/font再構築を追加しない。
- stale metricsとpixelの組み合わせ → generation一致と取得/更新消費の契約回帰を設ける。
- public API互換性 → legacy frameのstruct literalとserde回帰を維持する。
- consumer未接続 → KRRの実契約とKDV/KatanAの接続状況を別に記録し、単体合格を利用側全体の合格へ転用しない。

## Migration Plan

追加APIと両軸の実処理を同じreleaseへ含め、旧APIも維持する。仕様・回帰・性能結果を確認後に公開する。利用側の更新は各repoのIssueを先行させる。
