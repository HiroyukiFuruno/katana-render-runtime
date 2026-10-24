# SVGライブラリの採用・保守評価

2026-10-24のユーザー指示により、forkの作成許可と採用判断を区別する。修正効果だけでなく、公開版への追従速度、保守者の活動、代替ライブラリ、KRRでの実測を評価する。

## 採用判断の順序

fork公開許可を採用確定として進めた判断を是正する。まず代替の保守性・要件適合を一次資料と実装で比較し、有力候補はKRRへ接続して同じ入力で精度・表示までの時間・RSSを測る。条件を満たす保守負担の小さい代替があれば採用する。既存forkへの投資や検証済み修正の存在を、代替を却下する理由にしない。比較が未完了の間、forkの公開・registry依存への採用は確定しない。承認済みの差分保存と独立したKRR修正は継続する。

代替2方式をKRRへ接続して初期描画比較を実施した。ただし、font選択の完全一致、3 OS配布、表示までの時間・RSSは未確認であり、「代替がない」と結論しない。公開APIの違いは接続費用として調べ、未検証の仮説だけで不採用にしない。

## 数値による選定契約

「良さそう」「低負担そう」だけで採用しない。まず以下の必須条件を個別判定し、通過した候補の維持負担を比較する。任意の重み付き総合点で品質・性能・配布の未達を相殺しない。未測定は不明とし、0点・合格・代替不能へ換算しない。異なる母数の部分結果は順位確定に使わない。

| 評価軸 | 記録する数値 | 判定 |
| --- | --- | --- |
| 描画精度 | 試行・取得・失敗・採点・99以上の各件数、最低点、既存合格の脱落数 | 正式Mermaid117/117が99以上。部分cohortは探索結果として区別 |
| HTML互換 | 90以上の件数/全対象数、font不一致件数、consumer回帰失敗数 | HTML84/84が90以上、出力font一致、私有全領域等の既存条件も維持 |
| 表示時間 | 初回・再表示・resize別の秒数中央値・p95、同条件の対数、現行との差 | 性能非劣化を維持。3秒未満だけで劣化を合格にしない |
| メモリ | 同じ境界のpeak RSS bytes、現行との差、対数 | 非劣化。保持heapとprocess peakを混同しない |
| 配布 | 必須3 OSのbuild・実行成功数/3、追加native依存数、配布容量bytes | 必須3/3。macOSだけの成功を3 OSへ外挿しない |
| 維持負担 | adapter実装・テストのfile数/行数、独自patch数/追加削除行数、更新時に修正したpatch数、実作業時間 | 行数は代理指標。fork全体・CI・binding・将来の未実装部分を除いた小さな行数だけで低コストと結論しない |
| 上流保守 | 観測日、最終公開からの日数、期間内公開数、関連修正の取込み・公開までの日数 | stars/forksは補助指標として別記。件数や公開間隔だけで保守意思を推定しない |

### 2026-10-24の追加測定（採用未判定）

| 候補・測定範囲 | 試行/取得/採点 | 99以上/採点数 | 最低点 | 時間/RSS・3 OS |
| --- | --- | --- | --- | --- |
| Skia physical登録、既存12図（R2062/R2063） | 12/10/10、未接続font拒否2 | 0/10 | 66.902 | 未測定・未確認 |
| Pango FT登録＋librsvg/Cairo中間SVG＋Skia、英日flowchart（R2088/R2090） | 2/1/1、日本語font不一致拒否1 | 0/1 | 97.59019 | 未測定・未確認 |
| Pango FT登録＋librsvg/Cairo中間SVG＋Skia、既存12図（R2096/R2097） | 12/12/12、取得失敗0 | 0/12 | 82.5365 | 未測定・未確認 |

同じ英語flowchartのSkia比較値は90.981で、今回97.59019（+6.60919）。これは同一入力・基準・RGB scorerの部分改善であり、12件や117件の改善数へ加算しない。R2089は対象外のpaint projectionを空集合に実行して後処理が失敗した。失敗を保持し、projection対象でないflowchartは既存`scoreNativeProductRows`のRGB経路だけでR2090へ新規採点した。基準・閾値・画素は変更していない。

登録後の日本語個別font loadは、指定されたHiragino Sans W3の抽出SFNTと異なるblobを返した。R2092のloaded checksumは端末のW4 TTCのSHA-256と一致したため、font選択条件の差であり、日本語画像の精度不足へ読み替えない。R2087の登録checksum・設定復元・実文字描画の3回帰は成功。実SVG各glyphの使用face証明は別に未完了。

公式Fontconfigの`FONTCONFIG_FILE`で比較子processだけの設定を明示し、system font directoryを含めず選択済みfaceをPangoへ登録した。R2094/R2095は英日2図とも取得・採点し、97.59019/97.72696で99以上0/2だった。R2096/R2097の同12図は全取得・採点成功、99以上0/12、最低82.5365・最高97.72696、寸法失敗0。配色別評価のC4/TreeViewも既存の正式scorerを変更せず実PNGで採点した。source/binary不変を確認し、build後の差分許容は明示した2文書とlockだけでsource code差分を許可していない。fontconfigの設定path/SHAは各runへ保存した。英日2図から全12図へ拡大しても品質条件は未達であり、この接続のままの本番採用は認定しない。時間/RSSは測定していないので性能の優劣は不明のままとする。[設定仕様](https://fontconfig.pages.freedesktop.org/fontconfig/fontconfig-user.html)

現在の評価専用adapterは、Skia関連4file 487行、librsvg複合関連4file 475行、ThorVG関連3file 385行（各test込み、共通正規化・selector・依存設定は含まない）。承認済みusvg3patchは追加93行・削除14行。集計境界が異なるため、これだけでforkの方が安いと結論しない。未実装のfallback/variation、CI/native配布、上流追従と公開運用の費用は不明として残す。

検索語: `quantified alternative selection`, `unknown is not pass`, `no weighted waiver`, `fork last resort`。

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

librsvgの既定Pango font mapにはKRRの物理fontを束縛していない。端末の公開PangoCairo APIを実queryした結果、既定font typeはQuartz（enum 3）で、Quartz mapの作成も可能だった。PangoFT2の存在やFontconfigの検索結果を、この実描画backendの選択証拠として扱わない。R1924のusvg事前解析auditも実Pango描画のface観測へ代用しない。同一fontでの比較が未確定なので、0/12をライブラリ本体の精度不足の証明にはしない。

公開`pangocairo::FontMap::set_default`による間接注入経路があり、同梱GIRはPango 1.32.6以降のdefaultがthread単位と明記している。librsvgがdefault mapからcontextを作る以上、専用setterがlibrsvgに無いだけでfork必須やprocess全体の競合と結論しない。実使用face/index/variationの観測と、設定を戻すlifetime契約はなお未検証である。

R1942/R1943は既存usvg writerの解決済みSVGを同じSkia DOMへ渡した追加対照で、独立基準は変更していない。CSS解決の前処理を加えても12件すべて未達となり、CSS selector対応だけで置換可能になる仮説を採用しない。文字のbaseline・shaping・font選択、canonical再parseの意味差は未解消である。この正規化は候補経路の中間表現であり、正解画像の置換ではない。

### 追加対応と継続保守の比較

- Skia SVG DOM: 公開CPU SDKで実build・描画できたため、binary不存在を理由に却下しない。一方、現行Skiaのtag factoryに`style`要素がなく、inline style属性だけではMermaidのCSS selectorを処理できない。既存CSS cascadeからのmaterializeを検討できるが、それだけの改善幅は未測定。DOM builderの既定shaperはPrimitiveで、Rust wrapperにHarfBuzz factoryを渡す接点がないため、shaping接続にはbinding追加などが必要。物理fontの選択・観測、3 OSのfont providerも追加評価が必要で、単純な既存lib差替えではない。
- librsvg: Pangoの既存layoutを使えるが、現行adapterの画像は12件とも99未満。`text.rs`の`add_pango_attributes`はfamily/style/weight/stretch/sizeを渡し、variationを明示伝播していない。公開rendererのfont注入・実使用face/index/variation取得の接続は未確認。下流から観測できないという断定はせず、Pango/Cairo層の接続調査とnative依存の3 OS配布費用を残す。画像や入力座標の恣意的な補正で埋めない。
- LunaSVG: pinned v3.5.0 READMEでfilter非対応を確認。現行KRRのfilter回帰を削除せず代替するには追加対応が必要であり、現時点で低負担の置換候補とは認定しない。実性能は未測定。
- ThorVG: 公式v1.1.3とRust同梱v1.1.2のSVG text builderはweight 600以上をoutlineで擬似bold化する。現行の物理bold faceを選択する契約との差異候補だが、公開font登録・aliasと既存の入力正規化で回避できるかは未確認であり、loader修正やforkが必須とは断定しない。物理faceを固定した400/600/700の小入力で公開APIによる選択を先に検証する。KRR組込み・性能は未測定。
- 限定usvg fork: 既存tree/glyph/face APIとKRR合成を維持し、上流差分は承認済み3修正に限定できる。画像1,730件を含む標準fork回帰はR1915で成功した。ただし本家更新への追従、3修正の再評価、元版へ戻す責任をKRR側が負う。代替より優れているとする最終判断には、この負担も含める。

現在の判断は「追加開発なしで置換できる有力2候補は、KRR内の初期品質比較で条件未達」。限定forkの比較検討を続ける根拠にはなるが、未測定の性能・全117合格や本番採用を認定する根拠にはしない。公開・registry切替は採用理由の確定後に行う。

ThorVGにはsafe Rust wrapper `thorvg 0.5.1` があり、raw FFIを独自実装する必要はない。`Thorvg::load_font_data`のaliasとSVGのfamily名を一致させる接続は公開APIで可能で、物理bold fontを別aliasへ登録する経路を除外しない。ただしTTC face index・variation指定はこのAPIの引数に無く、単独static fontのPoC成功だけではKRRの全font契約を認定できない。外部libraryによるface抽出等の追加費用を比較し、公開接点がないことだけを「代替不能」の証明にしない。API根拠は[公式safe wrapper](https://docs.rs/thorvg/0.5.1/thorvg/struct.Thorvg.html)。

### librsvgの幾何・文字処理とSkiaの描画を組み合わせる候補

公開APIだけでlibrsvgからCairo SVG surfaceへ出力し、その中間SVGを既存Skia DOMへ渡す評価経路をKRRへ組み込んだ。R1974のbuildはactual0だったが、R1975の小数倍率・premultiplied RGBA共通回帰は失敗したため、12図の比較へ進めていない。

R1980で同じ4×3赤矩形の中間SVGと実画素を記録した。Cairoは全面paintをviewportより大きいrect（x=-0.4、y=-0.3、width=4.8、height=3.6）へ変換し、Skiaの既存接続ではviewport外まで描くため、1.25倍時の右下が期待する部分被覆ではなくRGBA `[255, 0, 0, 255]` となった。中間SVGの寸法は4px×3px、viewBoxは0 0 4 3で変わらない。この結果は接続境界のclip欠落を示し、librsvg自体の精度不足や代替不能の根拠にしない。期待値を維持して境界を修正・再検証する。実使用font・精度・時間・RSS・3 OS配布の判断は未完了である。

outline専用のviewport clipを追加し、R1981のAST/標準foundationとR1982のbuildはactual0、R1985は既存直接Skia経路を含め3/3回帰成功となった。右下RGBAは `[191, 0, 0, 191]` で既存許容範囲内。診断stdoutで個別結果が改行された1件を当初の単行counterが数え落としたため、元rawとcatalogから3件を照合した訂正を`result-corrected.json`へ別保存し、元結果も保持した。R1983はcatalog suffixが2件に一致して選択を拒否した事前失敗であり、テスト成功へ転用しない。

R1984/R1986で同じ12図を取得・採点した結果、99以上は0/12、範囲81.32560〜97.74015、寸法失敗0だった。同じ12図の既定経路は8/12、93.61936〜99.93882。入力・基準・scorerは同じだが、候補のPango実使用fontの一致は未証明であり、この初期品質未達だけで代替不能と判断しない。品質未達のため時間・RSS比較はまだ行っていない。取得と採点はactual0・ソース不変で、証跡は`tmp/quality/svg-scaled-font-product-probe-r1984/`と`svg-scaled-font-score-r1986/`。製品既定経路、正解画像、合格点は変更していない。

### 図形描画だけをSkiaへ渡す追加候補

全面置換とは別に、既存usvg解析・glyph描画・clip合成を維持し、solid pathの描画だけ既存Skiaへ渡す小さなadapterを検証した。R1947のAST/標準foundation、R1948 build、既定経路のR1949回帰193件は成功。R1950/R1951の12件を、同じsource/binary・入力SHAとbackendを照合してR1952へ再利用し、残105件だけ新規取得した。R1953では64/117が99以上（現行62/117からXY bar-line英日2件が新規合格、旧合格脱落0）。最大改善は1.37371点だった。

ただし、候補を有効にしたR1954回帰は191/193で、fractional clipの同色背景不変条件と既存stroke比較が失敗した。前者は実際の合成不変条件の破れであり、後者も期待値を無断変更して通過させない。部分的に精度が上がっても、この状態では既定へ採用しない。時間・RSSも未測定。現行既定の合格数は62/117のままであり、候補64件を製品合格として報告しない。元画像・失敗ログ・全採点は`tmp/quality/svg-scaled-font-score-r1953/`と`svg-scaled-font-regressions-r1954/`に保持する。

R1959でこの不変条件を3入力へ分離すると、候補のclipなしAA矩形だけ66pixelが30から29へ変化し、clip付きと背景だけは不変だった。R1960の独立Chromiumはclip付き・clipなし両方で66pixelが変化した。候補のclipなし画像もChromiumとは62pixel差があり、完全一致とは認定しない。したがって「clip maskの混在が主因」「同色保存がChromiumの描画契約」という推論は採用しない。既存回帰の失敗は保持し、ブラウザとの差と数学的な不変条件を区別して扱う。独立比較は`tmp/quality/svg-scaled-font-regressions-r1960/paired-pixel-comparison.json`。

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

### 追加実測と判断の訂正（2026-10-24）

修正パッチの有効性とfork採用の妥当性は別の判断である。公開許可は比較工程の省略を認めるものではない。比較完了前にforkを前提として準備を進めたことは判断誤りとして扱い、採用・公開は未確定のままとする。

未接続のfont条件を残した初期scoreで代替不能と結論しない。PangoのFT font mapへの実face登録を用いる候補は、librsvg/Cairoによる中間SVGとSkiaによる最終描画の複合経路として評価する。Pangoが個別にloadしたfont blobのchecksum一致は登録確認であり、SVG実描画の全glyphがそのfaceを使用した証明にはしない。まず英日flowchart各1図の同条件画像を取り、有望なら既存cohortへ拡大する。採用判断では精度だけでなく表示時間・RSS・3 OS配布と継続保守負担を比較する。

ThorVGは評価専用featureでKRRへ接続した。R2007では公開clip APIによる端数境界とstatic TTFの登録前後を含む5検査が成功した。R2008のnative回帰は2,595成功・失敗0・ignore18。ただしTTC、variation、bold、fallback、全画像スコア、3 OS、性能の比較は未完了であり、採用条件を満たしたとは認定しない。証跡は`tmp/quality/svg-scaled-font-regressions-r2007/result.json`および`tmp/quality/svg-scaled-font-regressions-r2008/result.json`。

R2012の独立baseline/candidate計30プロセス比較では、操作fixtureのcandidateに表示時間約15〜27ms・ピークRSS約13〜29MiBの増加が残った。これは現行候補と公開0.4.23の比較であり、ThorVG自体の性能結果ではない。3秒未満を非劣化条件の代わりにせず、再生成されるfont manager/typefaceを調査する。証跡は`tmp/quality/html-paired-interaction-diagnostic-r2012/result.json`。私有原本・84項目全体の正式性能合格には転用しない。

既存Skia managerのthread内再利用後、R2029の同じ30プロセス診断で再表示0.00238〜0.00268秒・resize0.00319〜0.00375秒となった。初回とRSS差は残り、runtime全回帰2,597件成功を性能合格の代用にしない。clip合成の冗長コピー削減も実装したが、この操作fixtureでRSS改善は確認できていない。Skia FontMgrを維持してDataの借用lifetimeを破る最適化は採用しない。

Skia SVG DOMの現候補はまだ`FontMgr::default()`を直接渡している。既存公開APIの`textlayout::TypefaceFontProvider::register_typeface`と`utils::OrderedFontMgr::append`により、選択済みの物理Typefaceを先頭、system managerを後段へ登録する経路がある。次の比較では同じSVG・実font bytes/indexを用い、default/provider/provider+fallbackの差を確認する。alias登録だけで元family/style解決やglyph別fallbackの完全一致を証明したことにはしない。現在の0/12を、この未接続経路まで含めた代替不能の根拠にしない。

R2036で上記の限定probeとcrop回帰2件が3/3成功した。Trebuchet MSのnormal/boldを実source bytes/indexへ照合して登録し、両文字領域にinkがあること、登録したphysical Typefaceへのstyle matching、provider-onlyとprovider+systemの全pixel一致を検証した。default managerとは実pixel差がある。R2033の`new_from_bytes`経路はsource Dataを取得できず失敗を保持し、同じbytesを保持する`new_from_data(Data::new_copy)`へ変更して厳格照合を維持した。静的R2034・build R2035も成功。ただし明示aliasのcontrolled probeであり、元family/CSS・TTCの非0 index・variation・mixed fallback・12図の再採点や性能は未証明（`tmp/quality/svg-scaled-font-regressions-r2036/result.json`）。

過去R1942/R1943は`resolved-text.svg`を用い、既存writerは`preserve_text=true`を明示している。アウトライン化された文字を比較していたわけではない。Skia DOM自体にはstylesheet/class selectorの同等処理を確認できず、CSS解決済みのpresentation属性を渡した経路として区別する。次の登録font比較もこの入力境界を維持する。

R2047の登録provider診断は12件中10件取得・C4英日2件SIGSEGVだった。Skiaの実sourceでは、SVG textのfamily検索失敗時にnull familyへ再問い合わせし、OrderedFontMgr経由のTypefaceFontProviderがnullを文字列検索へ渡していた。元crash reportと失敗を保持して、usvgが解決した要求familyの先頭を公開provider APIで事前照合する形へ修正した。SkiaのSVG parserがfamily一覧の先頭しか使わないことも一次sourceへ照合した。未解決をsystem fallback成功へ読み替えず明示拒否する。

providerはfamily名の大小文字を区別していたため、実familyに加えて小文字aliasを登録し、登録前の拒否→同じ物理face登録後の解決を回帰へ追加した。R2059 AST/foundation・R2060 build・R2061重点23件が成功。R2062/R2063は同じ12図を試行し、10図取得・99以上0/10、C4の2図はOpen Sans未接続の明示拒否、SIGSEGV0だった。採点できた10図のscoreは旧正規化Skia経路と全件同値（66.902〜95.27442）。入力・基準・scorer・閾値は不変。物理font登録だけで差が解消する仮説は支持されないが、glyph別fallback/variationの完全一致や、未接続2件の品質を証明した結果ではない。ThorVGの実font/TTC/bold接続と他候補の維持費の判断を残し、fork採用・公開はまだ確定しない。証跡は`tmp/quality/svg-scaled-font-product-probe-r2062/result.json`と`svg-scaled-font-score-r2063/scores.json`。

R2073ではThorVGへ実Trebuchet MS normal/boldの別source SHA・index0を別aliasで登録し、同canvas/text/fontsizeの4条件（regular/bold alias各400/700）が非空ink・premultiplied RGBA・寸法の契約を満たした。全6組合せのpixel差分は258/280/343/319/342/261だった。公式実装ではweight600以上をoutline化し、SVG familyを単一FontLoaderへ渡す。複数familyを登録してもglyph単位のfallbackはなく、TrebuchetとHiraginoの混在にはrun分割・CSS継承・位置・kerningを再現するadapterか上流変更が必要となる。TTCは既存write-fontsによるface抽出で登録可能なので、index引数不足だけを却下理由にはしない。これは4入力の接続評価であり、ThorVGの全12/117品質・性能を測った結果ではない。

librsvg 2.63.3の実sourceは既にHintStyle::None・HintMetrics::Off・Pango glyph position rounding無効を設定していた。単にCairo側のhint設定を追加して改善する仮説は棄却し、不必要な同一条件の再試験は行わない。既存のlibrsvg/Cairo→Skia 0/12に対して、物理font観測と3 OSのnative依存維持は未解決のままである。

R2073の全runtime回帰2,605件は成功。R2078の新しいHTML84診断は83成功の安全な再利用と不足1件だけの再取得で84/84が90以上（最低91.79849）、font完全一致を標準readerが確認した。ただし公開依存・私有全領域・性能非劣化の合格ではない。CoreText-onlyとSkia-onlyの準備込み比較は同一環境4対で約53/66ms・RSS約21.25/24.28MiBだったが、2glyph専用でpaintの計測境界も異なるため、CoreTextの製品採用を決める根拠にはしない。比較の範囲と不足を維持したうえで、代替adapterの追加実装・3 OS配布負担と、限定3patchの継続保守負担を採用判断で比較する。

- resvg方針・貢献・ライセンス: https://github.com/linebender/resvg
- 最新公開版: https://github.com/linebender/resvg/releases/tag/v0.48.1
- 最近の修正取込み例: https://github.com/linebender/resvg/pull/1164 、https://github.com/linebender/resvg/pull/1154
- Skia Rust SVG DOM: https://github.com/rust-skia/rust-skia/blob/0.153.3/skia-safe/src/modules/svg/dom.rs
- librsvg保守・機能・配布: https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/index.html
- LunaSVG: https://github.com/sammycage/lunasvg 、https://github.com/sammycage/lunasvg/releases/tag/v3.5.0
- ThorVG SVG対応範囲: https://github.com/thorvg/thorvg/wiki/SVG-Support
- librsvg text資料の対象版: https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/text_layout.html
