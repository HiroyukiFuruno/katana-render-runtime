## Context

v0.1.4 は、v0.1.0 transfer と v0.1.2 runtime asset version pinning の後続である。

v0.1.0 の正本は renderer / exporter / score の移植であり、v0.1.2 の正本は runtime asset の version 固定と更新管理である。v0.1.4 はその上に、実際に画面を開いて表示結果を確認する E2E を追加する。

画面上では、左に reference 出力、右に kcf 出力を並べ、上部で case を切り替え、拡大縮小や背景色切替を行う。下部にはファイル名、寸法、score report への path を表示する。

## Goals

- SVG / PNG / JPEG / PDF / HTML を実ウィンドウで確認できる
- reference と kcf 出力を左右比較できる
- スクリーンショットを `tmp/viewer-e2e/screenshots/` に保存できる
- 自動 smoke で「起動できる、入力を読める、表示が空でない、スクリーンショットを保存できる」を確認できる
- 手動目視確認の観点を artifact に残せる
- `floem` / `egui` などの画面表示依存を `crates/` に入れない

## Non-Goals

- ImageMagick score の正本を変更しない
- score の閾値や baseline policy を変更しない
- viewer e2e を公開 CLI として固定しない
- KatanA UI の代替 preview を kcf に実装しない
- v0.2.0 以降の viewer rendering 本体を先取りしない

## Placement

推奨配置は `test/e2e/viewer/` とする。

- `test/e2e/viewer/Cargo.toml`: workspace 非参加の単独検証器
- `test/e2e/viewer/src/`: 実ウィンドウ、case 読み込み、表示、スクリーンショット保存
- `test/e2e/viewer/cases/`: Mermaid / Draw.io / export の case 定義
- `test/e2e/viewer/README.md`: 実行方法と目視確認観点
- `tmp/viewer-e2e/`: 生成物、スクリーンショット、report

workspace 非参加にする理由は、通常の `cargo test --workspace` や publish package に画面表示依存を混ぜないためである。

## Candidate Comparison

| 候補 | 良い点 | 悪い点 | 判断 |
| --- | --- | --- | --- |
| Rust 画面表示ライブラリ（Floem） | Rust だけで実ウィンドウを作れる。KatanA の次期検討にもつなげやすい | CI の画面起動が重い可能性がある | 第一候補 |
| Rust 画面表示ライブラリ（egui） | 現 KatanA に近い確認ができる | egui 依存を延命しやすく、移植後の疎結合方針と衝突しやすい | 初期採用しない |
| ブラウザ自動操作（Playwright） | スクリーンショットと CI が安定しやすい | native window ではない | HTML / screenshot 補助 |
| OS 標準ビューア（OS native viewer） | 実利用に近い | 自動化と再現性が弱い | 手動確認専用 |

## Recommended Architecture

初期実装は Floem を使う E2E 専用 viewer とする。

viewer は kcf library を直接呼ばず、事前に生成された SVG / PNG / JPEG / PDF / HTML と reference artifact を読む。これにより、core library と画面表示層の依存方向を切り離す。

PDF は初期段階では直接描画を必須にしない。PDF から画像化された artifact を表示できればよい。PDF 直接表示は未決事項として扱い、必要なら v0.1.4 実装前に固定する。

## Just Recipe Design

少なくとも次を追加する。

- `just viewer-e2e-open case=<name>`: 指定 case を実ウィンドウで開く
- `just viewer-e2e-screenshot case=<name>`: 指定 case のスクリーンショットを保存する
- `just viewer-e2e-smoke`: 最小 case を起動し、非空表示とスクリーンショット保存を確認する

recipe は `test/e2e/viewer/` を明示的に指定して実行し、workspace の通常 test と混ぜない。

## CI Policy

継続的統合（CI）では、初期は Linux の smoke のみを候補にする。

CI の必須条件は、native window を安定して起動できることが確認できてから release gate に入れる。安定しない場合は手動確認用 recipe として残し、CI 必須化は後続 task に分離する。

## Verification Boundary

自動検証で扱うもの:

- viewer process が起動する
- case 定義を読める
- reference / kcf artifact を読める
- 表示領域が空でない
- スクリーンショットを保存できる
- process が非0終了しない

手動目視で扱うもの:

- 文字欠け
- ラベル切れ
- 線の重なり
- 余白の崩れ
- 背景色の違和感
- 実ウィンドウで見たときの差分

score 判定は ImageMagick compare を正本にし、viewer e2e は採点しない。

## Version Dependencies

- `v0.1.0`: Mermaid / Draw.io / export / score の transfer が前提
- `v0.1.2`: Mermaid.js / Draw.io.js の version 固定が前提
- `v0.1.4`: 実表示 E2E を追加
- `v0.2.0`: CSV viewer rendering の確認 case を viewer e2e に追加できる
- `v0.3.0`: PDF viewer rendering の確認 case を viewer e2e に追加できる
- `v0.4.0`: Word / Excel / PPTX viewer rendering の確認 case を viewer e2e に追加できる

## Risks

- CI で native window 起動が安定しない可能性がある
- Floem のスクリーンショット保存方式を事前に確認する必要がある
- PDF を直接表示するか画像化して表示するかを固定する必要がある
- viewer e2e を release gate に含める時期を決める必要がある
- case 定義が増えすぎると、手動確認の負荷が高くなる
