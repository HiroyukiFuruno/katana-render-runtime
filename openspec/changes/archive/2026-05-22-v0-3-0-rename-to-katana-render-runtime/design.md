## Context

KDV の HTML export は Markdown AST から数式や図形を評価できるが、PDF / PNG / JPG 側で同じ成果物を使えないと、raw TeX、raw diagram source、theme 差分、サイズ差分が発生する。KDV 側に MathJax 実行基盤を個別に持つ近道はあるが、既に KDR が V8 runtime と generated bundle を管理しているため、実行基盤を重複させるより repository の責務を広げたほうが正しい。

ただし、数式は図形ではない。したがって `katana-diagram-renderer` の名前のまま数式を追加するのではなく、repository と primary crate を `katana-render-runtime` に rename する。これにより、図形、数式、将来の別 runtime を「描画成果物を生成する runtime」として扱える。

## Goals / Non-Goals

**Goals:**

- GitHub repository と primary crate を `katana-render-runtime` に rename する
- 既存履歴を維持し、KDR の v0.2.0 から連続する `v0.3.0` として release する
- `katana-diagram-renderer v0.3.0` は互換 wrapper として残す
- Mermaid / Draw.io / PlantUML の既存 runtime と API 能力を落とさない
- MathJax v4 系を使い、TeX input を SVG output へ変換する
- MathJax bundle は generated runtime artifact として repository / crate package に含める
- theme / dark-mode / diagnostics / asset 解決を render runtime 共通 contract として扱う
- KDV が HTML / PDF / PNG / JPG で同じ SVG を利用できる API を提供する
- README のロゴ、アイコン、badges、crate名、説明、migration note を更新する

**Non-Goals:**

- Markdown parser を `katana-render-runtime` に持たせない
- `$...$` / `$$...$$` / fenced `math` の Markdown 構文判定を runtime 側で行わない
- 受け取った入力の AST 解析を runtime 側で行わない
- KDV の export surface、pagination、PDF / PNG / JPG 生成責務を runtime 側へ移さない
- 旧 crate を crates.io から削除しない
- `katana-diagram-renderer` 名のまま数式 renderer を増やさない

## Decisions

### 1. Repository rename を正本にする

GitHub repository は `katana-diagram-renderer` から `katana-render-runtime` へ rename する。GitHub の rename は履歴と issue / PR の参照を維持できるため、新規 repository へコピーするより安全である。

Repository rename 後、README と docs は `katana-render-runtime` を正本名として扱う。旧名は migration note と compatibility section に限定する。

### 2. Version は KDR から引き継ぐ

これは新規 product ではなく、KDR の責務拡張と rename である。したがって新 crate は `v0.1.0` から始めず、現 KDR の次の major/minor line として `v0.3.0` から公開する。

移行後の version line は次の形にする。

```text
katana-diagram-renderer v0.2.0
        ↓ rename / generalize
katana-render-runtime   v0.3.0
katana-diagram-renderer v0.3.0 互換 wrapper
```

### 3. 旧 crate は互換 wrapper として残す

crates.io では crate を削除できない。`katana-diagram-renderer v0.3.0` は `katana-render-runtime v0.3.0` を re-export する wrapper とし、docs と README で移行先を案内する。

旧 crate に新機能を直接実装しない。実装の正本は `katana-render-runtime` に置く。

### 4. MathJax は render runtime の一機能として扱う

MathJax は TeX input から SVG output を生成する runtime として扱う。数式は図形ではないが、V8 bundle 実行、asset 管理、theme / dark-mode、diagnostics、SVG寸法正規化は Mermaid / Draw.io / PlantUML と同じ基盤に乗せられる。

Runtime API は Markdown source ではなく、正規化済み TeX を受け取る。KDV は `$...$`、`$$...$$`、fenced `math` を Markdown AST / KMM DTO として識別し、外側の marker を剥がして trim 済み TeX を渡す。

互換補助として単一数式 wrapper の normalize helper を追加する場合でも、それは Markdown parser ではなく、`"$ E = mc^2 $"` のような単一入力から外側 marker を剥がす小さい helper に限定する。

Runtime は受け取った入力の AST 解析をしない。Mermaid / Draw.io / PlantUML / MathJax のいずれも、consumer が指定した renderer kind と input string を受け取り、成功時は SVG、失敗時は diagnostics 付き raw string を返す。raw string fallback は runtime の責務として返すが、上位の成果物で成功扱いするか、診断として表示するかは consumer が決める。

### 5. 出力は SVG を第一成果物にする

`katana-render-runtime` は HTML / PDF / PNG / JPG を直接生成しない。第一成果物は SVG と寸法 metadata である。SVG化できない場合は raw string と diagnostics を返し、runtime 内で別表現へ隠蔽しない。

KDV は次のように利用する。

- HTML: SVG をそのまま埋め込む
- PDF: 同じ SVG を PDF surface に描画する
- PNG / JPG: PDF または同じ surface 経由で画像化する

この境界により、HTML と PDF / PNG / JPG の数式・図形差分を減らす。

### 6. Theme contract は complete object を優先する

上位 consumer は complete theme object と dark-mode を runtime request に渡せる。runtime 側は欠けた配色を暗黙 fallback で補完しない。

CLI など簡易入口では light / dark preset を持てるが、library API は complete theme を受け取る contract に寄せる。

### 7. README ロゴとアイコン更新を完了条件に含める

Rename 後も README のタイトル、説明、ロゴ、アイコン、badges が旧 KDR のままだと、crates.io と GitHub 上の認識が崩れる。README 更新は単なる装飾ではなく、公開 package の contract として扱う。

更新対象は少なくとも次を含む。

- repository title
- crate name badges
- README hero logo / icon
- package description
- install snippet
- migration note
- old crate compatibility section

## Risks / Trade-offs

- [Risk] 旧 KDR consumer が混乱する -> 旧 crate wrapper と README migration note で移行経路を明示する
- [Risk] MathJax を runtime に入れることで責務が広がりすぎる -> Markdown parsing と export surface は KDV に残し、runtime は SVG成果物生成に限定する
- [Risk] V8 bundle と MathJax bundle の package size が増える -> generated artifact と checksum を管理し、package 内容を検証する
- [Risk] theme contract が図形と数式で分岐する -> complete theme object と dark-mode を共通 request に入れる
- [Risk] rename と機能追加が混ざる -> phase を分け、rename / wrapper / docs を先に固定してから MathJax runtime を追加する

## Migration Plan

1. OpenSpec change を作成し、rename / version / wrapper / MathJax scope を固定する
2. `katana-render-runtime` の package metadata と workspace crate 構成を設計する
3. 旧 `katana-diagram-renderer` crate を wrapper 化する方針を実装前にテストで固定する
4. README、ロゴ、アイコン、badges、migration note を更新する
5. Mermaid / Draw.io / PlantUML の既存 tests と package checks が新名でも通る状態にする
6. MathJax runtime bundle を追加し、TeX to SVG の contract test を先に作る
7. theme / dark-mode / diagnostics / dimension metadata を MathJax output に反映する
8. `katana-render-runtime v0.3.0` を crates.io に publish する
9. `katana-diagram-renderer v0.3.0` wrapper を publish する
10. KDV 側へ dependency switch を handoff する

## Completion Gate

この change は、実装が通っただけでは完了にしない。完了条件は `katana-render-runtime v0.3.0` と `katana-diagram-renderer v0.3.0` wrapper の crates.io 公開まで含む。

MathJax については、TeX input から SVG output を返す focused test と、失敗時に diagnostics 付き raw string を返す focused test が通ることを必須にする。KRR は受け取った input string を処理するだけで、Markdown AST 解析や export surface 生成を担当しない。
