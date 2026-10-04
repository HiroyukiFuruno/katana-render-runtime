## Context

v0.4.23の配布検証はビルド・パッケージ内テストを通過したが、runtime crateが10MiBを超えた。主因は公式Draw.io JSの圧縮サイズ増加で、既存Brotli品質11の再圧縮は同じbytesとなる。計測原本は `tmp/issue95-package-size-root-diagnosis.json`。

## Goals / Non-Goals

**Goals:** 最新Draw.ioを維持し、全配布crateを現行容量制限内に収める。展開後checksumと既存描画回帰を維持し、公開レジストリ依存だけで動く。

**Non-Goals:** JSの再実装・再minify、CPU最適化の追加試行、容量上限・品質・coverageの緩和、他repositoryの修正。

## Decisions

- 同一workspaceの資産専用crateへ圧縮JSのみを移し、runtimeの既存Brotli sourceへ静的sliceを渡す。既存キャッシュ・展開・一時ファイル処理は維持する。
- runtimeからassets crateへのregistry依存は同一release versionへ完全一致させ、runtimeのDraw.io version/checksumと資産bytesの対応を固定する。次releaseでは両crateのversionと依存要件を同時に更新する。
- 資産のcompressed pathを生成導線の一箇所で定義し、更新時と同期検査で使用する。生JSとchecksumは既存catalogを一次情報として維持する。
- 3つの公開crateを同じworkspace versionに揃え、assets→runtime→CLIの順に公開・registry可視性確認する。
- 未公開workspace依存を含む配布検証は、まず最小fixtureでCargoのstagingを実証する。その後workspace内の公開packageをまとめて検証する。検証省略フラグは使用しない。
- 古いmajorへの恒久固定は更新の継続性を損ねる。再圧縮は実測で無効なため採用しない。

## Risks / Trade-offs

- 未公開依存を単独packageした際の解決失敗 → 最小fixtureと実配布packageの両方でregistry用lockfileと検証ビルドを確認する。
- 資産更新時の二重コピー・同期漏れ → 圧縮資産の一次保存先を一つにし、内容・checksum・package収録回帰を標準ゲートへ接続する。
- 部分公開 → 公開済みpackageをスキップして依存順に再開する既存retry規約を3packageへ拡張する。

## Migration Plan

最小Cargo fixture、資産移動と依存参照、release契約更新、全package実測・描画回帰、最新HEADレビュー、保護merge、依存順公開、公開版consumer検証、Issue閉鎖とcleanupの順に実施する。

## Open Questions

最小fixtureでworkspace選択と明示的2package選択の両方が実package検証ビルドを通過した。Cargo生成の一時registryを使用し、配布manifestにはpathを残さず、lockfileは通常crates.io source/checksumとなる。原本は `tmp/issue95-assets-workspace-packaging-probe/logs/`。package後の単独lib testで同じstagingを使う方法は追加検証中であり、実配布3packageの確認まで配布準備完了とは認定しない。
