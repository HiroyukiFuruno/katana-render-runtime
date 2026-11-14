# HTML fidelity feature inventory

この一覧は、2026-10-06時点の `katana-render-runtime` のソースと既存テストを直接確認して作成した bounded inventory である。catalog の6 visual caseと5 script caseは観点を固定するための入口であり、全CSS仕様・全DOM仕様・全ブラウザ互換を意味しない。`implemented` はコードと既存unit/contract testで実装入口が確認できる状態、`verified` は同じHTML・viewport・操作stateをChromeとKRRで比較したcatalog resultがある状態、`unverified` はpair結果が無い状態、`unsupported` は明示的に拒否または実装対象外であることを示す。

## 一次ソースと判定規則

| 領域 | 一次ソース | 確認した責務 |
| --- | --- | --- |
| HTML tree | `crates/katana-render-runtime/src/html_dom/{mod,tree,tree_sink,serialize,types}.rs` | parse、element/attribute/text、tree生成、serialize |
| CSS parse/cascade | `crates/katana-render-runtime/src/renderer/backends/html_css_{parser,rule,selector,selector_match,selector_parse,cascade,resolution,apply,shorthand}.rs` | declaration、selector、specificity、cascade、custom property、interactive/static mode |
| CSS generated content | `crates/katana-render-runtime/src/renderer/backends/html_css_{generated_content,pseudo}.rs` | `::before`/`::after`の文字列・escape・状態付きstyle |
| HTML layout/paint | `crates/katana-render-runtime/src/renderer/backends/html_interactive/{layout_flow,layout_inline,layout_table,layout_grid,layout_paint,svg}*.rs` | box、inline、table、flex、grid、文字metrics、SVG paint |
| DOM runtime | `crates/katana-render-runtime/src/renderer/backends/html_runtime/{session_runtime,execution,dom_state*,interaction,promise,script,style}.rs` | V8 script、DOM lookup/mutation、event、style、promise/budget |
| browser-facing behavior | `scripts/html/{fidelity-suite,interaction-suite,scroll-coverage,toc-fidelity}.ts` | Chrome PNG、scroll、TOC、interaction state |

`implemented` と `verified` を混同しない。catalog resultが無い項目は `unverified` のままにし、unit testだけでChrome/KRRの受入済みとは判定しない。

2026-10-07の保存証跡監査では、mandatory 84条件の厳密な再利用可能tupleは0件だった。入力状態18条件・script15条件には同一HTML SHA/viewportのKRR記録と補助Chrome PNGが存在するが、独立Chrome expected-state/source bindingがないため受入証跡ではない。以下の過去の`verified`や「Chrome同一操作state比較済み」という記述は操作時の観測を表すにとどまり、永続化されたChrome/KRR契約検証済みへ昇格させない。これらの現在の受入状態は`unverified`とする。残るvisual51条件にはこの補助組合せも確認できなかった。

## CSS/style inventory

| feature / property | source symbol | catalog case | normal state | boundary / operation state | status / gap |
| --- | --- | --- | --- | --- | --- |
| color、font-family、font-style、font-weight、font-size、line-height、text-align、text-decoration | `static_snapshot_property` in `html_css_resolution.rs` | `typography` | 通常の段落・見出し | 日本語/英語混在、小数metrics、nowrap | `implemented`（static snapshotの対象。Chrome/KRR pairと全font fallbackは未検証） |
| margin、paddingのlonghand/shorthand | `expand_box_shorthand`, `parse_declarations` in `html_css_shorthand.rs` / `html_css_rule.rs` | `layout` | boxの余白 | longhandとshorthandのcascade順 | `implemented`（既存unit test。visual全viewportはcatalog result待ち） |
| background、background-color | `static_snapshot_property` / `html_css_apply.rs` | `decoration` | surface背景 | gradient、重なり、opacity | `unverified`（gradientのChrome/KRR score未登録） |
| selector tag/class/id/universal、descendant、child | `CssSelector::parse`, `matches_at` in `html_css_selector*.rs` | `layout`, `decoration` | 通常selector | compound、ancestor、空白node | `implemented`（selector unit tests。catalog visual未実行） |
| attribute selector | `CssAttributeSelector`, `parse_attribute` | `input-state`, `navigation` | `[disabled]`等 | scheme URL、quoted value | `implemented`（既存selector tests。広いattribute grammarは未検証） |
| `:root`, `:disabled`, `:checked`, `:hover`, `:not(:disabled)` | `matches_state`, `matches_at_pseudo_state` | `input-state` | disabled/checked state | hover・focus遷移、keyboard | `unverified`（hover/focusの実Chrome/KRR pair未完） |
| `:nth-child`, `:first/last/only-child`, `:*-of-type` | `CssNthExpression`, `matches_at_with_type` | `table`, `layout` | sibling order | text nodeを除くelement order、overflow算術 | `implemented`（unit tests。全組合せは未検証） |
| `::before`, `::after`, `content`文字列/escape | `parse_generated_content`, `pseudo_style_at_state` | `decoration` | generated text | escape、invalid codepoint、static mode | `unverified`（static modeでpseudo styleは明示的に無効） |
| CSS custom propertyと`var()` fallback/cycle | `resolve_css_variables`, `inherited_custom_properties` | `decoration`, `typography` | inherited custom value | nested fallback、cycle、missing variable | `implemented`（cascade tests。layout propertyへの全適用は未検証） |
| media query width | `media_query_matches`, `media_width` | `layout` | viewport width | narrow viewport、複数条件 | `implemented`（interactive CSS tests。全visual viewport score未登録） |
| display/block/inline/inline-block | `html_interactive/layout_flow*.rs`, `layout_inline*.rs` | `layout`, `typography` | normal flow | mixed inline line、zero-font line | `unverified`（実装・regressionは存在するがcatalog result未登録） |
| flex / inline-flex | `html_interactive/layout_inline_atomic*.rs`, `layout_flow*.rs` | `layout`, `table` | card、badge | baseline、center/end、wrap | `unverified`（badge regressionあり。catalog visual未実行） |
| grid | `html_interactive/layout_grid_measure.rs`, `layout_grid_track.rs` | `layout` | two-column grid | min-content、narrow width | `unverified`（source path存在、Chrome/KRR pair未完） |
| table、cell、colspan/rowspan、border-collapse | `html_interactive/layout_table*.rs`, `layout_table_cell.rs`, `layout_table_content.rs` | `table` | header/body rows | min-content、rich child、collapsed conflict | `unverified`（table diagnostics/regressionは存在、catalog result未登録） |
| border、border-radius、box-shadow、opacity、clip/gradient paint | `html_interactive/svg*.rs`, `layout_paint*.rs` | `decoration` | surface/alert | paint order、clip、translucency | `unverified`（property別score未登録） |
| border-style solid/dashed/dotted/none、辺別指定 | `CssBorderStyle`, `style_border_properties.rs`, `svg_border*.rs` | `decoration`, `table` | 実線・破線・点線 | none→solidの指定幅保持、style省略、辺別style、丸角 | `unverified`（CSS/描画回帰あり。独立Chromeの各画像99は未達） |
| 相対font-sizeと小数font metrics | `css_relative_px`, `css_font_size`, `effective_html_font_size` | `typography` | em/rem/百分率の数値解決 | 小数サイズ、overflow拒否、author family順序 | `unverified`（数値回帰あり。全体画像99は未達） |
| UA入力幅と配置 | `html_input_character_metrics`, `layout_input_geometry.rs`, `style_text_metrics.rs` | `input-state`, `layout` | primary fontの平均幅・最大幅からsize属性を解決 | 無効平均幅の代替、absolute/fixedとrelative/stickyの行高さ | `unverified`（実状態18件一致。保存Chromeとの画像18件は全て99未満） |
| position fixed/sticky、overflow、scroll geometry | `html_interactive/layout_flow*.rs`, `scroll-coverage.ts` | `navigation` | fixed sidebar | full Y range、bottom clamp、TOC target | `unverified`（scroll manifestは別証跡、catalog PNG pair未登録） |
| unsupported CSS at-rules / unknown declarations | `html_css_parser.rs` (`parser_skips_unsupported_at_rules`) | all visual | ignored syntax | malformed/unsupported rule | `unsupported`（明示的にskip。ブラウザと同一挙動の全種類は未検証） |

## DOM/script inventory

| API / behavior | source symbol | catalog case | normal state | boundary / operation state | status / gap |
| --- | --- | --- | --- | --- | --- |
| `querySelector`, `querySelectorAll`, `matches`, `closest` | `dom_state_lookup.rs`, `html_runtime_interaction_tests.rs` | `script-dynamic-dom`, `events` | id/class/compound lookup | no match、detached node、attribute selector | `supported`（既存Rust tests。新catalogのKRR result未取得） |
| `createElement`, `appendChild`, `remove`, tree mutation | `dom_state_mutation.rs` (`mutate_tree`) | `script-dynamic-dom` | append/remove child | detached target、invalid operation | `supported`（既存 interaction tests。catalog pair未取得） |
| `textContent`, `innerHTML`, markup/content lookup | `dom_state_content_lookup.rs` | `script-dynamic-dom` | read/write text | nested markup、resource presence | `supported`（既存 tests。全serialization差分は未検証） |
| attributes、class、inline style read/write | `set_attribute`, `style`, `html_runtime/style.rs` | `script-dynamic-dom`, `input-state` | set/remove attr、style property | camelCase→kebab-case、duplicate declaration | `supported`（unit tests。操作後PNG未登録） |
| click/input/submit/keyboard event dispatch | `interaction::event`, `event_flags` | `events`, `input-state` | listener and form operation | capture/bubble、cancelable、defaultPrevented | `supported`（event contract tests。全keyboard sequence未検証） |
| `addEventListener` options (`once`, capture) / custom event | `html_runtime_lifecycle_tests.rs`, `html_runtime_tests.rs` | `events` | listener ordering | once、remove、exception | `supported`（既存lifecycle tests。catalog実行結果未取得） |
| readyState、DOMContentLoaded、load、readystatechange | `session_runtime.rs`, `html_runtime_lifecycle_tests.rs` | `async`, `resources` | parser/lifecycle order | image/iframe callback、handler replacement | `supported`（Rust lifecycle tests。fixture KRR pair未取得） |
| Promise/microtask/timer and execution budget | `promise.rs`, `execution.rs`, `session_runtime_interactive.rs`, `html_browser/session.rs` | `async` | Promise、microtask、host-driven timer deadline | infinite loop、budget exhaustion、delay/cancel/navigation/closed session | `implemented`（deterministic logical clock）。`HtmlBrowserSession::advance_time(Duration)`実host回帰PASS、wallclock自動実行は未提供 |
| image/data resource validation and load/error | `dom_state.rs` (`validateImageDataUrl`), `html_subresources` | `resources` | valid data image | malformed/remote/error resource | `supported`（data URL tests）。network/CORS/font resourceは未検証 |
| layout metrics, bounding rect, intersection metadata | `dom_state_geometry.rs`, `session_layout_metrics_intersection_tests.rs` | `observers`, `navigation` | rect lookup | rotation、empty/non-finite、scroll | `unverified`（bridge/API testsあり。catalog state pair未取得） |
| `IntersectionObserver` | `dom_bootstrap.js`, `session_layout_metrics.rs`, `session_layout_metrics_intersection_tests.rs` | `observers` | observe target、layout refresh | refresh after scroll, hidden/removed target | `verified`（`tmp/html-fidelity/dynamic-krr-public-host/observers.json` state pair。全API境界は未検証） |
| `MutationObserver` | `dom_bootstrap.js`, `dom_state_mutation.rs` | `observers` | attributes/childList callback、takeRecords、disconnect | subtree、options、delivery ordering | `verified`（同KRR/Chrome state pairでtakeRecords/disconnectを実行。oldValue/filter全域は未検証） |
| `ResizeObserver` | `dom_bootstrap.js`, `session_layout_metrics.rs` | `observers` | observe target、layout refresh callback | resize delivery/order、unobserve/disconnect | `verified`（同KRR/Chrome state pairでresize deliveryを実行。box-size全域は未検証） |
| `scroll` / `resize` host events | `interaction::dispatch_window_event`, session event tests | `navigation`, `observers` | window event dispatch | scroll-induced mutation、resize | `supported`（host dispatch入口）。全viewport dynamic pair未検証 |
| unsupported DOM operation | `HtmlDomBridgeState::dispatch/lookup/mutate_*` | all script | unknown operation | explicit error | `unsupported`（explicit contract error; browser API全域を代替しない） |

## catalog case coverage and remaining gaps

| catalog case | covered source families | required normal/boundary/operation state | current evidence boundary |
| --- | --- | --- | --- |
| `typography` | text metrics、font properties、inline wrap | mixed script、fractional line metrics、nowrap、long text | Chrome visual fixtureは存在。KRR score未登録、font fallback全域は未検証 |
| `layout` | box、flex、grid、media width | narrow、intrinsic、multi-column | source/unit tests存在。catalog全viewport pair未実行 |
| `table` | table sizing、rich cell、border conflict | min-content、span、mixed content、collapse | dedicated regression存在。catalog pair未実行 |
| `navigation` | anchor、fixed sidebar、scroll geometry | all Y、bottom、全link click | `scroll-coverage`の別manifestで16 TOC/3 viewport確認済み。catalog pair未実行 |
| `input-state` | input/select/details、state pseudo | empty/filled/selected/expanded/disabled/focus | fixtureは存在。KRR interactive resultと操作後PNG未取得 |
| `decoration` | background、radius、shadow、gradient、clip、opacity | stacked/clipped/translucent | fixtureは存在。property別99 score未取得 |
| `script-dynamic-dom` | DOM mutation/content/style | add/remove/attribute/text/class/style | Chrome expected state実評価済み。KRR同一state未取得 |
| `script-events` | event listener、capture/bubble、form | input/click/submit/keyboard | `tmp/html-fidelity/dynamic-krr-public-host/events.json`でKRR state PASS、Chrome同一操作state比較済み。PNG RMSE99未達は別gate |
| `script-async` | promise、microtask、timer、ready | order and eventual state、host deadline | `tmp/html-fidelity/dynamic-krr-public-host/async.json`でKRR state PASS、Chrome同一操作state比較済み。遅延host APIは別回帰 |
| `script-observers` | mutation/intersection/resize | mutation、scroll、resize、observer delivery | `tmp/html-fidelity/dynamic-krr-public-host/observers.json`でKRR state PASS、Chrome同一操作state比較済み。全Observer API網羅は未検証 |
| `script-resources` | data image、load、font ready | load/error/resource addition | Chrome expected state実評価済み。KRR resource pair未取得 |

この文書は「catalogのcaseを実行済み」とは宣言しない。次の昇格条件は、各行について同一HTML SHA・viewport・操作stateでChrome/KRR state JSONとPNGを保存し、feature resultへ全featureを`passed`で登録すること。未対応APIは実装へ推測で拡張せず、`unsupported`または`unverified`のままIssue/DoDへ戻す。
