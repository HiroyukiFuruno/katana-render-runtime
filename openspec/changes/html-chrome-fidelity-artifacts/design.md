## Context

Issue #106はChrome描画・操作互換性と旧#107の永続化契約を所有し、#108はMermaidを所有する。製品HTML runtimeはRust/V8、描画はCSS/layout→SVG→RGBAである。私有資料はローカル読取とignored出力だけに限定する。

## Goals / Non-Goals

**Goals:** 各独立画像99以上、semantic/interaction/performance各95、provenanceで安全に無効化できるhost保存、再起動後のfresh操作session、全品質・レビュー・公開確認。

**Non-Goals:** JS heap/DOMやフォーム状態のsnapshot復元、Chromeの製品依存化、私有資料の配布、自己比較による合格。

## Decisions

- Playwrightは実Google Chromeの参照PNGだけを取得する。KRR probeは別processでruntimeからPNGを取得し、元寸法のRMSE/MAEの最小scoreを使う。resize/cropや参照基準のKRR置換を拒否する。
- CSS author/UA originを別属性にして優先順位を保つ。auto intrinsic寸法はpadding/borderを常に加算し、指定border-box幅/高さとは区別する。font行高・baselineは使用font metricsから算出する。
- artifactはformat version/provenance/完全frameを持つ。sourceSHA、origin、runtime版、config/resource/fontSHA、viewport/scaleが一致した時だけhitにする。hostが保存を明示有効化しroot/quota/clearを所有する。
- 保存は同rootのcreate_new temporaryへ0600で書きsync後renameする。store lockでquota競合を直列化し、クラッシュ残留lockはhostが停止確認後明示復旧する。破損、容量、symlink、無制限readをtyped errorで拒否する。
- resumeは現版・現sourceを検証後Rust/V8を新規openしscrollを適用する。静止pixelsを操作可能sessionと偽らず、JSは再実行する。
- Mermaid参照ではKRR固有補正を無効にし、class node変換は構造で対象を識別する。

## Risks / Trade-offs

- pixels/originに機密内容が含まれる → host opt-inとprivate store ownershipを公開契約にする。raw HTMLや入力値を別fieldへ保存しない。
- font/resourceのhost fingerprintが不正確 → host責務と64hex必須を明記し、mutation missを検証する。
- filesystemの悪意ある並行置換 → symlink/ancestor検査、bounded read、host専有rootと明示lock recoveryを要求する。
- 独立ChromeのOS/font差 → 同一環境内で同viewport/scale/resources/fontsを揃え、各画像・状態を記録する。99未達を対応範囲の説明だけでcloseしない。

## Migration Plan

公開APIは追加として導入する。既存hostは保存disabledのまま動作できる。完全gate、Draft reviewと指摘対応、native CIの後v0.4.24公開版を独立検証しIssueを閉じる。未達なら公開・完了扱いせず修正を継続する。

## Open Questions

私有資料・Chrome画像99・Issue統合・全Issueリリースはユーザー承認済み。元semantic/interaction/performance各95の計算式・対象集合は既存Issue/コードから特定できておらず、参照先を照会中。新しい操作回帰の成功率や性能上限判定を、元契約の95点達成へ読み替えない。

### HTML文字描画の候補評価

#### 2026-10-23: 共通文字ラスタ修正の実装境界

公開Skiaの全SVG置換はR1560で105図中82図が悪化したため採用しない。SkiaSVGには中央揃え時に累積`dy`をY方向のanchor補正へ混入する処理があり、限定的な等価変換ではR1566のフローチャート英日・横向きブロックが99以上へ改善した。しかしmultiply等の既存合成やdominant-baselineにも差が残る。正解入力・参照PNG・採点条件は変更しない。

次の製品候補は、既存usvgの実glyph ID・物理font data/TTC index・確定済み配置をそのまま公開`skia-safe`のglyph描画APIへ渡し、形状・順序・clip・blendは既存compositorに保持する。SkiaSVGで再shapeしない。対象はstatic font、solid fill、stroke/装飾なしの通常文字へ限定し、variable font・色付きglyph・不明font・非対応paintは既存resvg経路を維持する。描画開始前にText全体の対応可否を確認し、途中まで描いた後にfallbackしない。予期しない失敗はエラーとして伝播する。

公開SVG APIのhost非依存経路は変更せず、HTML専用入口だけでglyph rendererを渡す。font/typefaceはrender単位で共有し、glyphごとのfont file読み直しやfont DB全走査を避ける。surfaceは既存frame上限・文字boundsで制限し、RGBA順序とpremultiplied alphaを明示する。既存画像との無劣化、117図99以上、HTML84と全実font一致、初回/再表示/resizeの表示時間とRSS非劣化、3対象OSのビルドを満たしてから採用を確定する。

現在のSVG→TinySkiaでは、実Chromeと同一font/文字位置でもglyphの描画差が残る。Skia CPU glyph描画をignoredの独立prototypeで評価する。製品採用は全18画像と原資料の各99以上、既存品質、3対象OSの配布可能性を確認して判断する。限定画像の成功で採用を確定しない。

採用する場合はfontdbが選択した実fileとTTC index、variation、Harfrustのglyph/GPOSを共有する。SVGのtextごとに元位置へglyph画像を置き、親のclip/opacity/transformと描画順を維持する。全文字を取り除いて最後に重ねる方式は採用しない。tspanの属性継承、文字間隔、dx、textLength、装飾、device scaleを保持し、画像allocationとrender-scoped cacheを上限内に制限する。最新互換依存を評価し、古い取得可能版へ固定するために対象OSや品質条件を下げない。

HTML専用候補はrasterize_html_svg境界へ限定し、一般SVGの既存出力を維持する。既存resolved_font_runsとHarfrustのbufferを同一font DBで利用し、face/index/source generation・glyph ID・cluster・offset・advanceを保持する。font-feature由来のSVG dxはnativeのfeature適用と二重計上しない。font data/typefaceはrender単位で共有し、毎runのfontdb走査・CJK全font copyを避ける。混在run内の空白のみtextも可視文字として保持する。採用前の全画像99と既存回帰の条件は維持する。

### 最新ユーザー是正（一次情報）

匿名fixtureは原本のDOM/CSS/JS、目次階層・順番・リンク先、表とカードの配置を維持する。会社/製品/個人名だけの置換では不十分で、業務内容・固有要件・実績・金額・日付・本文を架空の新内容へ全て書き換える。旧library架空レイアウトは原本構成の受入として採用しない。各viewportには実際の最大Yスクロール到達後の画像比較を含め、smooth scrollは条件到達を待つ。固定1800pxやscroll直後の画像を最下部の証拠にしない。

### 段階公開と複数fixtureの検証単位（2026-10-07）
ユーザー承認により先行段階#109は各画像90以上、最終#110は各画像99以上とし、親#106は最終達成までopenを維持する。両段階とも性能劣化は不可であり、旧95点という総合表記から非劣化達成を推定しない。

## ライブラリ委譲と再利用の検証契約

### 2026-10-24: HTML生成文字と埋め込みSVG文字の担当分離

R2174/R2175では既存resvg文字経路のHTML84画像がすべて90以上だった。一方、Skia FontMgrの初回取得にはR2165で約0.019秒を観測した。画像だけの診断で採用を認定せず、HTML serializer自身が生成した文字に限り既存resvgへ委譲する候補を実装する。Mermaid等の埋め込みSVGは現在のSkia経路を維持し、入力種別を画像スコアから推測して切り替えない。

由来はclipとは独立したprivate sidecarに保持する。生成したtextのIDとmarkup SHAを同時記録し、入力内・parsed tree内の一意性と完全一致を検証する。prefixだけでは信用せず、衝突・変更・ID移動は担当分離の対象外として従来Skia経路を維持する。画像とレイアウトを変えずにIDだけを付ける。objectBoundingBox等で既存compositor全体がresvgへ委譲する場合は個別Skia担当が作動したと報告しない。

採用前にHTML/埋め込みSVGの実担当・font証明、全117画像・HTML84と私有全領域、初回/再表示/resizeの時間とRSSを再評価する。resvg側のlayout済みfont情報だけを追加の実paint証明と偽らず、観測範囲と未証明部分を分ける。Skia fork、正解画像・閾値の変更、ユーザー向け設定追加は行わない。

文字整形・フォント表解析・metrics計算は既存ライブラリへ委譲する。委譲の確認は呼出先だけで終わらせず、KRRが所有するhandle、ライブラリ内の共有table/metrics、render scopeの寿命、無効化条件まで追跡する。独自の計測アルゴリズムを追加する前に、既存ライブラリの再利用機能と実際の呼出経路を検証する。

アンチパターンは、同じフォントを毎回再生成してライブラリの共有cacheを破棄し、画像scoreだけで採用を判断すること。正しい検証は、実フォントA→B→A→Bでライブラリmetricsの共有と描画値の一致を確認し、正常scope終了・file mutation・破損・計算前の保持容量制限も検査すること。cache所有分と現在の読込を合わせたsource/table budgetは既存64MiBを増やさず、外部callerが所有するhandleは別の所有範囲として明記する。

描画経路と測定経路は、同一DBという理由だけで同じフォントを選ぶと判断しない。familyの大文字・小文字、優先順、weight/style/stretch、実際のface/source/indexと出力値を照合する。正解画像・期待寸法を変更して計測差を隠さない。

生成済みJavaScriptを実行する経路では、性能計測のビルド前に既存の `runtime-bundle-check` を必須実行する。検査の失敗と検査中のソース変更を拒否し、未同期の生成物を含む候補のsnapshot・build・measurementを開始しない。RustとJavaScriptのAPI契約検査は、実際に呼ばれたCSS/SVG両経路を記録する。CSSの丸め値とSVGのraw metricsを混同せず、API契約の合格をブラウザ画像一致の証明として扱わない。

画像の合格と性能の合格は独立した必須条件とする。現在ソースに結び付いた初回・再表示・全スクロール・操作後の時間とmemoryを同条件で比較し、キャッシュ未命中・font表読込の観測で原因を説明する。古いソースの合格、0件テスト、未実行の計画、単一viewportの結果を全対象の合格へ読み替えない。検索語: library-owned cache, font handle lifetime, alternating faces, source budget admission, selected font parity, performance non-regression。

複数fixtureを一つのsource SHAへ押し込まず、case/state/viewport/scaleごとの検証単位でsource SHA・候補実バイナリ・独立Chrome参照・操作状態・対応する時間とメモリの証跡を検証する。catalogから必須集合を先に固定し、一件の欠落や不一致も受入不可とする。複数バイナリで得た画像を同一実行バイナリの結果と偽らない。既存単一fixture検査は保持し、suite集計は平均ではなく全検証単位の合否で決める。

診断counterは開始したrender scopeに帰属させ、別scope内や元scope終了後の記録を棄却する。nested enabled/disabled、終了順序、unwind、thread isolationを回帰で確認する。library呼出区間からSource準備とcounter更新を除外し、残るobserver overheadとinclusive区間を明記する。診断用ビルドの値を正式な性能比較へ転用しない。

フォントDBの診断は対象経路を明記する。HTMLのDB cache未命中が0でも、SVG raster用のtransient DB、SourceCache、SourceIdが再生成されていない証明にはならない。各経路のcache所有者と生成箇所を追跡し、反復描画時の実source取得を観測する。ライブラリcacheを呼出しごとに作り直す実装は、再利用を検証した実装として扱わない。

フォントライブラリの入力境界では、CSS custom propertyを宣言元で解決して継承する。fallbackを含む循環と置換後の不正なfont-familyはCSS契約に従って処理し、別フォントを推測して計測エラーを隠さない。実vendor図の要求とnative bridge結果を回帰で確認し、ブラウザ画像の正解を修正後の出力から作り直さない。
