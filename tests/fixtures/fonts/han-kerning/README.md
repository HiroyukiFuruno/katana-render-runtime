# Han 字詰め回帰用フォント

`NotoSansCJKjp-han14.otf` は、[Noto Sans CJK JP Regular 2.004](https://github.com/notofonts/noto-cjk/blob/523d033d6cb47f4a80c58a35753646f5c3608a78/Sans/OTF/Japanese/NotoSansCJKjp-Regular.otf) のサブセットです。元の固定 commit は `523d033d6cb47f4a80c58a35753646f5c3608a78`、元ファイルの SHA-256 は `68a3fc98800b2a27b371f2fb79991daf3633bd89309d4ffaa6946fd587f375b5` です。

サブセットの SHA-256 は `b0d6e5438ab6d2dca24a0632a91aa504fe76c087d879a4c739230ed78279496c`、サイズは 3,292 bytes です。U+0020、U+0028、U+0029、U+0041、U+2018、U+2019、U+201C、U+201D、U+3000、U+300C、U+300D、U+3042、U+FF08、U+FF09 の14文字と GPOS の `halt`・`palt` を保持し、`chws` は含みません。

HarfBuzz `hb-subset` 15.1.0 で作成しました。`SOURCE_FONT` は上記元フォント、`OUTPUT_FONT` は出力先を示します。

```sh
hb-subset "$SOURCE_FONT" --unicodes=0020,0028,0029,0041,2018,2019,201C,201D,3000,300C,300D,3042,FF08,FF09 --layout-features=halt,palt --output-file="$OUTPUT_FONT"
```

元フォントの著作権表示は © 2014–2021 Adobe です。元フォントと本サブセットは SIL Open Font License 1.1 です。対応する固定元のライセンス全文と著作権条件は同梱の `OFL.txt` を参照してください。

`NotoSansCJKjp-Regular-subset-chws.otf` は [WPT の固定 commit c2504f4062db9bed36540e28c49caae3e4fd873c](https://github.com/web-platform-tests/wpt/blob/c2504f4062db9bed36540e28c49caae3e4fd873c/fonts/noto/cjk/NotoSansCJKjp-Regular-subset-chws.otf) の原本です。サイズは 30,016 bytes、SHA-256 は `fa2f2a3e29179fbd4e258f3f3f988c2c7ab95979ae7d2297e2bd22b8f953067b`、Git blob は `18859d5e2fe1200da5caef5f88d15864d0477963` です。

[WPT の説明](https://github.com/web-platform-tests/wpt/blob/c2504f4062db9bed36540e28c49caae3e4fd873c/fonts/noto/cjk/README.md)では Noto CJK を chws_tool で処理してサブセット化したフォントとされています。処理元フォントと生成ツールの正確な版は不明です。このfixtureはWPTの固定binaryを改変せずに使用します。WPT 同梱の [LICENSE.txt](https://github.com/web-platform-tests/wpt/blob/c2504f4062db9bed36540e28c49caae3e4fd873c/fonts/noto/cjk/LICENSE.txt) は上記 `OFL.txt` と同一bytesで、SIL Open Font License 1.1 を適用します。
