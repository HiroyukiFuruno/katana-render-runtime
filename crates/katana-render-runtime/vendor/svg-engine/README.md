# SVG engine の内部ソース

このディレクトリは、`HiroyukiFuruno/katana-resvg` の承認済みcommit
`c01f0165059e472f45472cccddc573d32c13548a` から移植したライブラリソースである。
元となるupstreamは `linebender/resvg`。元ソースの著作権表示とSPDX識別子を保持し、
Apache-2.0 / MITのライセンス本文を同梱する。

`usvg/` は元の `crates/usvg/src/`、`resvg/` は `crates/resvg/src/` に対応する。
相対階層を保持した48個のRustファイルを収録し、両方の `main.rs` は含めない。
元ソース内の `cfg(test)` とテスト2件は保持する。別crateのmanifest、CLI、外部fixture、
crate外を参照するREADMEのsymlinkは持ち込まない。

KRR内部の `crate::svg_engine::{usvg,resvg}` として接続するため、次の機械的な適応だけを行った。

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
描画、字形処理、構文解析の処理内容とテストの期待値は変更していない。
`PROVENANCE.json` に、元commit、全ファイルの元パス・移植先・両SHA256・変更件数と、
ライセンスのSHA256を記録している。SHA256は各ファイルの全バイトから算出する。

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
