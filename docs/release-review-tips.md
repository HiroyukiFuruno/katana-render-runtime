# リリース検証・レビュー対応の実務メモ

## 個別修正の証拠

過去の「対応済み」返信と全体CI成功は、個々の不具合が直った証拠ではない。指摘の再現入力をcurrent HEADへ当て、可能な限り同じ回帰テストの修正前RED・修正後GREENを確認する。replyにはcommit・固有テスト名・検証条件を記録し、未実行のOSを検証済みと書かない。

Issueコメントだけでレビュー未到着と判断しない。formal reviewsとreviewThreadsも全ページ取得し、対象HEADと未解決事項を照合する。push後は旧HEADのレビューを流用しない。

## 長いゲートの前に

focused test、format、lint、ASTの責務境界を先に確認する。特にhelperやfixtureを追加すると、テストが成功していても関数長・ファイル長のゲートを超えることがある。閾値を緩めず、責務のまとまりで分離してから完全検証へ進む。

対象crateへの `cargo clippy -- -D warnings` だけをrepoのstrict lint成功と読み替えない。`just lint` は `clippy::too_many_lines` などを明示的に有効化するため、完全gate前の検査も既存入口を使う。raw JS文字列を返すRust fixtureも関数長の対象となる。

subagentの検証報告は、起動行や無出力を成功と推定したものを受理しない。toolの実終了コードと、テストなら実際の対象・件数を照合する。縮約ログで判定できなければ `CARGO='rtk proxy cargo'` と既存入口で生の結果を確認する。これは検査の省略やwrapperを失敗原因と決めつける根拠ではない。

coverageの未到達箇所は行・関数・regionを区別し、対象と同じソース版で特定する。テストhelperのerror closureも対象になるため、製品コードをcoverage都合で変える前に実際の未到達経路を確認する。

OS固有の探索を追加した場合、手元OSのcoverage成功をLinuxの成功と同一視しない。機能のないOSで常に空を返すstubと共通分岐を組み合わせると、到達不能行が生まれる。OSごとの実際の責務に合うcfg構造を確認し、coverage対象除外やテスト用の製品分岐で埋め合わせない。

圧縮資産を増やす変更では、配布サイズだけでなく最小入力の展開量・保持量も確認する。selectorより前の全展開やprocess-wide cacheは、圧縮サイズから見えない常駐メモリ増加を生む。独立グループの非選択時に展開しないことを契約テストへ固定し、展開後データの寿命と配布上限も併せて検証する。

DOM移行では通常childrenだけでなくtemplate専用Documentのような別格納先を確認し、要素全体とChildrenOnlyの両入口で入れ子・順序・escapingを回帰化する。Observerは交差フラグだけでなくratio/rectも同時に検証し、Element rootとDocument rootを分ける。DOM ancestryとlayoutのcontaining-block chainは同一ではないため、限定修正を完全仕様準拠と説明しない。

parser移行時は通常の文字列だけでなく、置換なしtemplate literalのcooked値など旧ASTで対応していた構文形も照合する。状態準備の回帰は、その状態を副作用で準備する別イベントを使わない（例: layout-ready検証にscroll refreshを混ぜず、Click経由のlate observeで確認する）。新しいJS評価を追加したら、既存のtimeout時runtime破棄契約も全入口へ適用する。

Web APIの入力検証は、API固有の範囲検査とWebIDLの型変換を分けて一次仕様と照合する。IntersectionObserverでは有限の範囲外thresholdはRangeError、NaN/Infinityはdouble変換のTypeErrorである。Array.mapが疎配列の穴を飛ばすこと、Numberがboxed BigIntを受理することも検証漏れになるため、正常値・空列・数値文字列・疎配列・変換拒否を実runtime回帰へ固定する。根拠: [Intersection Observer初期化](https://www.w3.org/TR/intersection-observer/#initialize-new-intersection-observer)、[WebIDL double](https://webidl.spec.whatwg.org/#es-double)。

成功済みの重い描画比較を再利用する場合、証跡元SHA・結果・変更call chainを固定し、最終差分が監査済みの非描画pathだけであることを機械照合する。HTML observerや検査器の限定修正でも、依存lock・vendor/asset・生成bundle/checksum・diagram renderer・共有V8/font/SVG・外部rasterizerのいずれかが変われば再利用しない。閾値や比較対象を減らすのではなく、同じ描画入力と実装に対する既存証跡を引き継ぐ。変更したHTMLを含む完全release-checkとbundle不変検査は新HEADで必ず実行する。

圧縮境界だけを変える場合は、上記の非描画path判定とは別に資産同値性を証明する。旧commitと新生成物をそれぞれindexに従って展開し、全資産の順序付き `(path, raw bytes)` 列、重複・欠落なし、連続offset・展開長・archive末尾を照合する。内容だけ一致しても、JSのfirst-match検索で注入順変更が描画へ影響するため不十分である。selector、描画実装、元資産、依存lockが不変で、この全列の同値性を証明できた場合に限り、任意selectorによる部分列も同一として描画証跡を引き継げる。圧縮生成物の整合、配布サイズ、完全release-checkは改めて検証する。

## RSS測定と並列テスト

プロセス全体のRSS差分には同時実行中の別テストの割当ても入る。coldメモリ検査は同じtest executableを専用子プロセスで1件だけ実行し、他テストの並列性を保ったまま測定対象を隔離する。終了成功だけでなく、完全修飾名のテストが実際に1件PASSしたことを確認する。RSS・所有データ量などの受入閾値は変更しない。

## コマンドと証跡

引数を取るjust recipeを続けて列挙すると、次のrecipe名が前の引数として解釈される場合がある。例えば描画比較は `just mermaid-compare-full 99` と `just drawio-compare-full 99` を個別に呼ぶ。

長時間コマンドのログは`pipefail`付きで保存し、実際の終了コードと成果物の数値を併せて確認する。zshの予約変数`status`を終了コード保存に使わない。描画比較は参照画像の再生成と区別し、基準を更新して差分を隠さない。

通常pushが無出力でも、lefthookのoutput=failureでpre-pushのjust checkが実行中の場合がある。remote-helperのstdin待ちやCLOSED socketだけで通信障害と断定して中断しない。対象の親Gitとhook子孫を限定して確認し、hook終了コードとremote HEADを照合する。hookを無効化して再試行しない。
