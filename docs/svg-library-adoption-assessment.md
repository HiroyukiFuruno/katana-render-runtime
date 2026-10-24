# SVGライブラリの採用・保守評価

2026-10-24のユーザー指示により、forkの作成許可と採用判断を区別する。修正効果だけでなく、公開版への追従速度、保守者の活動、代替ライブラリ、KRRでの実測を評価する。

## 採用判断の順序

fork公開許可を採用確定として進めた判断を是正する。まず代替の保守性・要件適合を一次資料と実装で比較し、有力候補はKRRへ接続して同じ入力で精度・表示までの時間・RSSを測る。条件を満たす保守負担の小さい代替があれば採用する。既存forkへの投資や検証済み修正の存在を、代替を却下する理由にしない。比較が未完了の間、forkの公開・registry依存への採用は確定しない。承認済みの差分保存と独立したKRR修正は継続する。

代替2方式をKRRへ接続して初期描画比較を実施した。ただし、font選択の完全一致、3 OS配布、表示までの時間・RSSは未確認であり、「代替がない」と結論しない。公開APIの違いは接続費用として調べ、未検証の仮説だけで不採用にしない。

## KRR内での同一12図の初期比較

英日それぞれのflowchart・継承class・C4・timeline・tree view・journeyを選び、同一の前処理済みSVG、独立raw JS基準、同一scorerで比較した。合格基準99は維持する。描画寸法取得にはusvg parseを残す混成adapterであり、全面置換の性能を示す実験ではない。

| 描画経路 | 99以上 | score範囲 | 証跡 |
| --- | --- | --- | --- |
| 既定経路の診断（usvg解析と既存合成） | 8/12 | 93.61936–99.9388 | `tmp/quality/svg-scaled-font-score-r1927/scores.json` |
| librsvg 2.63.3 / Cairo | 0/12 | 81.5464–97.8105 | `tmp/quality/svg-scaled-font-score-r1925/scores.json` |
| Skia SVG DOM 0.153.3 | 0/12 | 62.4178–88.298 | `tmp/quality/svg-scaled-font-score-r1932/scores.json` |
| usvg正規化SVG → Skia SVG DOM | 0/12 | 66.902–95.27442 | `tmp/quality/svg-scaled-font-score-r1943/scores.json` |

各capture・採点はactual exit 0、ソース不変、寸法エラー0。現行12画像は以前のR1890とbyte一致した。候補adapterのfont監査をusvgの事前解析結果で代用していない。Cairoの初期alpha変換は修正し、両adapter共通のpremultiplied RGBA・小数寸法回帰はR1930の2件、R1935の190件内でも成功した。初期12画像は全pixelが不透明であり、変換修正で初期scoreが変わらないことを確認した。

この結果から、現時点の接続では代替2方式とも品質条件を満たさない。追加対応をすれば到達不能と証明したものではない。品質段階で不合格のため性能比較へは進めておらず、遅い・メモリが多いという結論には使わない。

R1942/R1943は既存usvg writerの解決済みSVGを同じSkia DOMへ渡した追加対照で、独立基準は変更していない。CSS解決の前処理を加えても12件すべて未達となり、CSS selector対応だけで置換可能になる仮説を採用しない。文字のbaseline・shaping・font選択、canonical再parseの意味差は未解消である。この正規化は候補経路の中間表現であり、正解画像の置換ではない。

### 追加対応と継続保守の比較

- Skia SVG DOM: 公開CPU SDKで実build・描画できたため、binary不存在を理由に却下しない。一方、現行Skiaのtag factoryに`style`要素がなく、inline style属性だけではMermaidのCSS selectorを処理できない。既存CSS cascadeからのmaterializeを検討できるが、それだけの改善幅は未測定。DOM builderの既定shaperはPrimitiveで、Rust wrapperにHarfBuzz factoryを渡す接点がないため、shaping接続にはbinding追加などが必要。物理fontの選択・観測、3 OSのfont providerも追加評価が必要で、単純な既存lib差替えではない。
- librsvg: Pangoの既存layoutを使えるが、現行adapterの画像は12件とも99未満。`text.rs`の`add_pango_attributes`はfamily/style/weight/stretch/sizeを渡し、variationを明示伝播していない。公開rendererのfont注入・実使用face/index/variation取得の接続は未確認。下流から観測できないという断定はせず、Pango/Cairo層の接続調査とnative依存の3 OS配布費用を残す。画像や入力座標の恣意的な補正で埋めない。
- LunaSVG: pinned v3.5.0 READMEでfilter非対応を確認。現行KRRのfilter回帰を削除せず代替するには追加対応が必要であり、現時点で低負担の置換候補とは認定しない。実性能は未測定。
- ThorVG: 公式v1.1.3とRust同梱v1.1.2のSVG text builderはweight 600以上をoutlineで擬似bold化する。現行の物理bold faceを選択する契約との違いを直すにはSVG loader側の対応が必要。font aliasやface抽出の可能性は排除しないが、それだけでweight選択差が解消すると扱わない。KRR組込み・性能は未測定。
- 限定usvg fork: 既存tree/glyph/face APIとKRR合成を維持し、上流差分は承認済み3修正に限定できる。画像1,730件を含む標準fork回帰はR1915で成功した。ただし本家更新への追従、3修正の再評価、元版へ戻す責任をKRR側が負う。代替より優れているとする最終判断には、この負担も含める。

現在の判断は「追加開発なしで置換できる有力2候補は、KRR内の初期品質比較で条件未達」。限定forkの比較検討を続ける根拠にはなるが、未測定の性能・全117合格や本番採用を認定する根拠にはしない。公開・registry切替は採用理由の確定後に行う。

### 図形描画だけをSkiaへ渡す追加候補

全面置換とは別に、既存usvg解析・glyph描画・clip合成を維持し、solid pathの描画だけ既存Skiaへ渡す小さなadapterを検証した。R1947のAST/標準foundation、R1948 build、既定経路のR1949回帰193件は成功。R1950/R1951の12件を、同じsource/binary・入力SHAとbackendを照合してR1952へ再利用し、残105件だけ新規取得した。R1953では64/117が99以上（現行62/117からXY bar-line英日2件が新規合格、旧合格脱落0）。最大改善は1.37371点だった。

ただし、候補を有効にしたR1954回帰は191/193で、fractional clipの同色背景不変条件と既存stroke比較が失敗した。前者は実際の合成不変条件の破れであり、後者も期待値を無断変更して通過させない。部分的に精度が上がっても、この状態では既定へ採用しない。時間・RSSも未測定。現行既定の合格数は62/117のままであり、候補64件を製品合格として報告しない。元画像・失敗ログ・全採点は`tmp/quality/svg-scaled-font-score-r1953/`と`svg-scaled-font-regressions-r1954/`に保持する。

未採用の評価用feature・native依存・adapterは、最終配布へ無条件に残さない。評価ソースの履歴と生成証跡を区別して保存し、選択した製品経路だけで3 OSと完全ゲートを確認する。

## 現時点の確認結果

| 候補 | 公開版・開発活動 | 普及度 | KRRでの負担と未確認事項 |
| --- | --- | --- | --- |
| resvg/usvg | 最新v0.48.1は2026-08-02公開。約83日空いている一方、2026-10-20までPR取込みを確認。READMEはPRでの貢献を歓迎するが、公開間隔や採用期限の保証はない | GitHub API観測時star 4,154、fork 370 | 現行のtree/glyph APIと既存検証を利用できる。必要な3修正を限定forkへ移す候補。公開頻度の低さとfork更新責任は残る |
| Skia SVG DOM | KRR既定はSkia 0.153.3のpdf機能のみ。評価用svg/textlayout/webpを有効にし、対応する公開CPU SDKで実buildを確認 | Skia全体の普及度をSVG DOM個別の利用実績と同一視しない | KRRで12図と正規化後12図を実測し、ともに99以上0件。glyph/run・物理font/variation証跡への追加接続、時間・RSSは未確認。公開APIに観測がないことだけを採用不能の証明にしない |
| librsvg | GNOMEのサポート系列・公開手順がある。現端末のpkg-configは2.63.3、Cairo1.18.6、Pango1.58.2。Rust crateは2.63.4も公開済み | GitHubミラーのstar/forkを本体GitLabの普及度の代用にしない | KRRへRust実装2.63.3を組み込み12図を実測し、99以上0件。Cairo/Pango/GLib等の3 OS配布とfont証跡接続、時間・RSSは未確認。2.63.4のtext.rsは2.63.3とbyte同一だが、その版全体の描画成功とは扱わない |
| LunaSVG | 最新v3.5.0は2025-09-14公開。2026-10-21のpushはあるが、公開版への追従速度は別評価 | GitHub API観測時star 1,221、fork 186 | C++/PlutoVG経路。現行より公開頻度が良い候補とは確認できない。KRRでの精度・時間・RSSは未実測 |
| ThorVG | 公式SVG対応表は2026-08-15更新、SVG Tiny1.2を対象としtext/tspan・baseline・clip/mask/blend等を列挙 | 普及度と実描画品質を混同しない | C/C++接続とfont資源・選択観測の追加接続が必要。KRRでの精度・時間・RSSは未実測 |

star/forkは普及度の補助指標とし、保守品質の合格点へ直接換算しない。1か月以上公開版が更新されない場合は、修正が利用者へ届く速度の懸念として記録する。commit/PRの活動があっても公開版の遅れを解消したとは扱わない。

## forkの扱い

- 作成済みfork: https://github.com/HiroyukiFuruno/katana-resvg
- 対象Issue: https://github.com/HiroyukiFuruno/katana-resvg/issues/1
- 公開許可: 正常primary glyphの保持、線幅込みlayer bbox、実font metricによるCSS ex解決の3修正と、それに対応する2crate。会話内でユーザーが個別承認した。
- 採用確定・公開は未完了。限定forkは短期の修正配布経路として準備し、独自の機能開発へ拡大しない。
- KRRリリースごとの依存更新時に、本家の最新公開版・main・対応Issueを確認する。3修正が公開版で不要になれば、その版で既存回帰・117実PNG・HTML84・性能を再検証して本家依存へ戻す。
- 新しい上流版へ追従する際は、patch差分と適用理由を個別に再評価し、元ライセンス・帰属・対象APIを維持する。機械的なrebase成功だけを採用証拠にしない。
- 本家へのPRはリリースの追加条件にしない。既存Issue #486はfallbackの関連報告。#662は歴史的なbbox議論であり、今回のstroke layer欠落そのものが報告済みと断定しない。

## 置換判断の条件

最初の組込み候補はlibrsvg。診断専用feature `svg-librsvg-evaluation`で既存KRRのSVG入口へ接続し、既定経路は切り替えない。初期アダプターは出力寸法取得のusvg parseを残し、描画にはlibrsvgを使う混成経路である。寸法の同条件比較には利用できるが、追加parse時間を含むため完全置換の性能として結論しない。font入力と最終選択の一致も未証明であり、成功扱いの監査情報を捏造しない。

CargoのRustパッケージ`librsvg`はSVG処理自体をRustで実装し、system-depsでCairo/Pango/GLib/Harfbuzz/FreeType/libxml2等を使う。system librsvg本体へ単純にリンクするwrapperと誤認しない。2026-10-24のcrates.io APIと`cargo info`では2.63.4の公開を確認（2026-10-23T22:01:31Z）。最初の比較は現在のGNOME配布版と揃えた2.63.3で固定し、公開直前の候補版更新は別途評価する。

比較境界は現在のKRRがusvg/resvgへ渡す前処理後のSVGで揃える。`svg_rasterize_preprocess.rs`はforeignObjectを事前除去するため、候補のforeignObject非対応だけで却下しない。HTML全体を候補SVGライブラリへ要求して比較条件を不当に増やさない。既存合成・テキスト経路をどこまで維持/置換するかを明記する。

公式librsvgの`text_layout.html`は2.55.90時点の設計資料であり、その未対応一覧を現2.63.3の欠落機能の証明に使わない。2.60でdominant-baselineの基本対応が入っている。現行source・対応表・実描画で確認する。

代替候補が対応形式・font証跡・3 OS配布の面で有望な場合は、KRRへ実際に接続して同じ入力・資源・font・状態で測る。Mermaid117件の99、HTML84件の90、表示までの時間とRSSの非劣化を維持する。未実測の候補について「遅い」「省メモリ」「精度が高い」と結論しない。現時点では代替への本番切替を承認した状態ではない。

## 一次情報

- resvg方針・貢献・ライセンス: https://github.com/linebender/resvg
- 最新公開版: https://github.com/linebender/resvg/releases/tag/v0.48.1
- 最近の修正取込み例: https://github.com/linebender/resvg/pull/1164 、https://github.com/linebender/resvg/pull/1154
- Skia Rust SVG DOM: https://github.com/rust-skia/rust-skia/blob/0.153.3/skia-safe/src/modules/svg/dom.rs
- librsvg保守・機能・配布: https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/index.html
- LunaSVG: https://github.com/sammycage/lunasvg 、https://github.com/sammycage/lunasvg/releases/tag/v3.5.0
- ThorVG SVG対応範囲: https://github.com/thorvg/thorvg/wiki/SVG-Support
- librsvg text資料の対象版: https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/text_layout.html
