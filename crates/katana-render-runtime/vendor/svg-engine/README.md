# SVG engine の内部ソース

このディレクトリは、`HiroyukiFuruno/katana-resvg` の承認済みcommit
`c01f0165059e472f45472cccddc573d32c13548a` から移植したライブラリソースである。
元となるupstreamは `linebender/resvg`。元ソースの著作権表示とSPDX識別子を保持し、
Apache-2.0 / MITのライセンス本文を同梱する。

`usvg/` は元の `crates/usvg/src/`、`resvg/` は `crates/resvg/src/` に対応する。
相対階層を保持した48個のRustファイルを収録し、両方の `main.rs` は含めない。
元ソース内の `cfg(test)` とテスト2件は保持する。別crateのmanifest、CLI、外部fixture、
crate外を参照するREADMEのsymlinkは持ち込まない。

KRR内部の `crate::svg_engine::{usvg,resvg}` として接続するため、次の機械的な適応を行った。

- 元の `crate::` 参照を、各ソースの所属する内部モジュールへの参照へ変更した。
- resvgからのusvg参照を `crate::svg_engine::usvg` へ変更した。
  `resvg/lib.rs` の元の `pub use usvg;` は、同ファイルで使用する内部aliasに変更した。
- usvgのHarfRust参照を `svg_harfrust` aliasへ変更した。
  元のusvgが使う0.12系の依存とKRRの別用途の依存を区別するためである。
- KRRで有効な `text`、`writer`、`svgz`、`system-fonts`、`memmap-fonts`、
  `raster-images` のfeature条件は、既存の全既定feature選択に従って定数評価した。
  初回移植で `all()` に対応させた真の属性79件を除去し、
  `not(all())` に対応させた非選択分岐4件を除去した。
  非選択分岐はresvgのraster画像無効時、usvgのSVGZ無効時、text無効時の
  `ex` 長さ・親font-size解決の2箇所である。OS条件とtest条件は保持した。

KRRはこのSVGエンジンに対するoptionalな公開featureを元から持たず、
全既定featureを有効にしていた。内部化に伴って別のfeature契約を新設せず、
元commitで選択されていた処理だけを残す。未採用のfeature組合せの分岐を持ち込まないことは、
KRRの既存の有効動作を変えるものではない。

元から存在する `allow` 属性46個は原文のまま保持した。
KRRへの移植境界である `usvg/lib.rs` と `resvg/lib.rs` に限り、
実測指摘のうち、定数cfg解決後に残る外部Clippy指摘と `dead_code` / `unused_imports` を対象にした
限定 `allow` 属性を1個ずつ、計2個追加した。Clippy項目はusvgが22種類、resvgが18種類である。
resvgの `identity_op` / `too_many_arguments` / `upper_case_acronyms` / `wrong_self_convention` は、
元の属性による許可を保持し、追加リストでは重複させない。
これはKRRの `src/svg_engine.rs` に置いていた属性の移動であり、KRR自身の実装には適用しない。
`clippy::non_minimal_cfg`、一括の `all`、`forbid(unsafe_code)` の無効化は含めない。
初回移植後、独立Chromeとの比較と失敗回帰に基づき、次の限定的な修正を追加した。

- usvgの文字整形では、HarfRustへCSS文字サイズを渡し、サイズ依存のtrackingを適用する。
  字形ID・variation座標・原SVGを手作業で加工しない。
- radial gradientの書き出しでは、非ゼロの焦点半径 `fr` を保持する。
  再parseで焦点半径がゼロへ変わり色分布が失われる問題を、公開COLRv1フォントと
  正・ゼロ半径のSVG回帰で確認する。既定のゼロは省略し、既存の期待SVGは変更しない。
- feDisplacementMapでは、callerがfilter scaleとcanvas transformを適用した倍率を渡す。
  sampler内でscaleを重ねて掛けず、premultiplied mapのRGBだけstraight化してから変位量を評価する。
  これは本家 [Issue 1160](https://github.com/linebender/resvg/issues/1160) の修正に基づき、
  samplerの照合元は [このcommitのraw source](https://raw.githubusercontent.com/linebender/resvg/e1a4c92da6d354a41c402f57d65705ad632eba25/crates/resvg/src/filter/displacement_map.rs)
  である（commit `e1a4c92da6d354a41c402f57d65705ad632eba25`）。
- markerの暗黙viewport clipだけをhard-edgeにする。clip範囲とtransformを維持し、
  markerの子図形と通常のuser clipのアンチエイリアスは変更しない。
- 閉じたサブパスのCubicTo直後に置くmarker-midは、曲線終端接線と閉じ辺の接線の二等分方向へ向ける。
  SVG 2の規則から算出した期待角度を独立Chrome画像でも確認し、marker-endの既存経路は変更しない。
- HTML用の任意のfallback resolverへ、元の要求font属性を渡す。
  primary faceの物理weightで要求CSS weightを置き換えず、nested SVGにも同じresolverを渡す。
  未設定時は元のfallback resolverを使い、既定の選択処理は変更しない。

元のテスト2件と期待値は保持し、新しい回帰はKRR側へ追加する。
`PROVENANCE.json` に、元commit、全ファイルの元パス・移植先・両SHA256・変更件数と、
ライセンスのSHA256を記録している。SHA256は各ファイルの全バイトから算出する。

Diffuse/Specular lighting primitiveの入力参照もwriterで保持し、`SourceAlpha`と名前付き参照の
parse-write-parse後のモデル・描画が同じであることをKRR側回帰で確認する。

`xlink:href` の参照反復は、起点・直前ノードだけでなく既訪問ノードへの循環で停止する。
短い参照はインライン履歴で追跡し、履歴容量を超える場合だけ標準 `HashSet` へ移行する。

`image-rendering` は、既存CSS経路で扱う値をpresentation属性でも同じように受理する。
`feMorphology` は未指定・両軸ゼロ・負の半径で入力を維持し、片軸ゼロでは他方の軸だけを処理する。
独立Chromiumの属性/CSS計算値とフィルター画素比較に基づく修正で、参照画像は変更しない。

## 更新方法

1. 更新対象の元commitと、KRRで使うfeature集合・依存版を先に確定する。
2. 現在の全ファイルを `PROVENANCE.json` の移植後SHA256と照合し、
   KRR側で追加された差分があれば個別に保全・評価する。無差別に上書きしない。
3. 確定した元commitからライブラリソースを取得し、両 `main.rs` を除外する。
   元バイトのSHA256を記録してから、上記の内部参照・alias・feature適応を適用する。
   有効featureの真属性を外し、偽分岐は個別に確認して解決する。
   未知のfeature述語は既定値を推測して置き換えず、KRR側の接続担当へ返す。
4. 元ソースとの差分を、上記の適応と承認された処理変更に分けて確認する。
   著作権、SPDX、ライセンス、既存 `allow`、OS条件、テストと期待値の保持を照合する。
5. 全ファイルの元SHA256・移植後SHA256と変更内訳を台帳へ更新する。
   KRRの既存静的検査、関連回帰、完全描画比較・性能・coverageは主担当の正規入口で行う。

この移植バッチ自体ではCargo設定、KRRのmodule接続、品質ゲートを変更しておらず、
コンパイル・テスト・formatterは実行していない。
