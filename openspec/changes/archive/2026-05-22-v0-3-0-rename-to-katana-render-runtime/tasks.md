## 1. Scope Guard

- [x] 1.1 `git status --short --branch` を確認し、既存差分と rename change を混ぜない
- [x] 1.2 branch 方針を確認する。公開配布と crate rename を含むため、master直作業ではなく dedicated branch を使う
- [x] 1.3 `proposal.md`、`design.md`、`specs/**/*.md` を読み、rename / v0.3.0 / wrapper / MathJax の前提を確認する
- [x] 1.4 `katana-diagram-renderer` 名で数式を追加しないことを implementation note に残す

## 2. Repository And Package Rename

- [x] 2.1 GitHub repository rename 手順を確認する
- [x] 2.2 workspace primary crate を `katana-render-runtime` へ rename する
- [x] 2.3 Rust module / package metadata / docs の正本名を `katana-render-runtime` へ更新する
- [x] 2.4 crate description を「diagram」ではなく「render runtime」として更新する
- [x] 2.5 `katana-render-runtime v0.3.0` から公開する version policy を `openspec/project.md` と release docs に反映する

## 3. Compatibility Wrapper

- [x] 3.1 旧 `katana-diagram-renderer` crate を `katana-render-runtime` の wrapper として残す構成を決める
- [x] 3.2 wrapper crate は新 crate を re-export し、旧 consumer の最小移行を可能にする
- [x] 3.3 wrapper crate に新規実装を置かない構造をテストまたは lint で固定する
- [x] 3.4 `katana-diagram-renderer v0.3.0` を互換 wrapper として publish できる package metadata にする
- [x] 3.5 README と docs に旧 crate から新 crate への migration note を追加する

## 4. README Logo And Public Metadata

- [x] 4.1 README のタイトルを `katana-render-runtime` へ更新する
- [x] 4.2 README の説明文を render runtime の責務に合わせて更新する
- [x] 4.3 README のロゴ / アイコンを `katana-render-runtime` 向けに更新する
- [x] 4.4 README badges の crate 名、docs.rs、CI 表記を新名へ更新する
- [x] 4.5 install snippet を `katana-render-runtime = \"0.3\"` へ更新する
- [x] 4.6 旧 `katana-diagram-renderer` は互換 wrapper であることを README に明記する
- [x] 4.7 crates.io description、repository URL、documentation URL を新名へ更新する

## 5. Existing Runtime Preservation

- [x] 5.1 Mermaid runtime の public API と出力 contract を rename 後も維持する
- [x] 5.2 Draw.io runtime の public API と出力 contract を rename 後も維持する
- [x] 5.3 PlantUML runtime の public API と出力 contract を rename 後も維持する
- [x] 5.4 既存 reference score / fixture / runtime asset checks を新名でも実行できるようにする
- [x] 5.5 KatanA / KDV が必要とする `RenderInput` / `RenderOutput` / diagnostics を削らない

## 6. MathJax Runtime

- [x] 6.1 MathJax v4 系の bundle 取り込み方法を決める
- [x] 6.2 MathJax bundle を generated runtime artifact として repository 管理する
- [x] 6.3 MathJax bundle checksum を runtime bundle manifest に追加する
- [/] 6.4 MathJax JavaScript asset を既存 runtime asset と同じ checksum 管理対象にする
- [/] 6.5 `just mathjax-latest` と `just mathjax-update` を追加する
- [x] 6.6 TeX input を SVG output に変換する public API を追加する
- [x] 6.7 Markdown parser は実装せず、入力は正規化済み TeX として受ける
- [x] 6.8 必要なら単一 `$...$` / `$$...$$` wrapper の trim helper を追加する。ただし Markdown block parser にはしない
- [x] 6.9 inline math / display math / one-line display math のSVG出力 contract test を追加する
- [x] 6.10 `\\frac`、`\\int`、`\\sum`、上付き、下付きが raw text ではなくSVGになることを検証する
- [x] 6.11 MathJax実行失敗時に diagnostics 付き raw string を返す contract test を追加する
- [x] 6.12 Runtime が受け取った入力の AST 解析をしないことを API docs に明記する

## 7. Theme, Size, Diagnostics

- [x] 7.1 complete theme object と dark-mode を MathJax runtime request に渡せるようにする
- [x] 7.2 MathJax SVG の background は必要に応じて transparent にし、本文背景へ溶け込ませる
- [x] 7.3 SVG width / height / viewBox を返し、KDV が PDF / PNG / JPG で安定配置できるようにする
- [x] 7.4 MathJax失敗時は diagnostics 付き raw string を返す
- [x] 7.5 theme 欠落値を runtime 側で暗黙 fallback しない

## 8. Publish And Migration

- [x] 8.1 `katana-render-runtime v0.3.0` の package list を確認する
- [ ] 8.2 `katana-render-runtime v0.3.0` を crates.io に publish する
- [x] 8.3 `katana-diagram-renderer v0.3.0` wrapper の package list を確認する
- [ ] 8.4 `katana-diagram-renderer v0.3.0` wrapper を crates.io に publish する
- [ ] 8.5 GitHub repository rename 後の remote URL と docs links を確認する
- [ ] 8.6 KDV 側へ「新 crate 公開済み、dependency switch 可能」と handoff する

## 9. Verification

- [x] 9.1 `just check` を実行する
- [x] 9.2 `just runtime-bundle-check` を実行する
- [x] 9.3 `just runtime-asset-check` を実行する
- [x] 9.4 `just mermaid-compare-ci 99` を実行する
- [x] 9.5 `just drawio-compare-ci 99` を実行する
  - 2026-05-23 実行結果: `templates-uml-sequence_1` が 98.79 で未達。ユーザー確認により、この既知差分は release blocker ではない。
- [x] 9.6 PlantUML focused tests を実行する
  - 2026-05-23 実行結果: 参照 PNG を現在の公式 PlantUML CLI `1.2026.4` 出力で更新し、`plantuml-compare-ci 100` は minimum score 100.00 で通過。
- [x] 9.7 MathJax focused tests を実行する
- [x] 9.8 `cargo package -p katana-render-runtime --locked --list` を確認する
- [x] 9.9 `cargo package -p katana-diagram-renderer --locked --list` を確認する
- [x] 9.10 `./scripts/openspec validate v0-3-0-rename-to-katana-render-runtime --strict` を実行する

## 10. Handoff

- [ ] 10.1 変更ファイル、挙動差分、検証結果を日本語でまとめる
- [ ] 10.2 旧 crate wrapper と新 crate の publish 状態を明記する
- [ ] 10.3 KDV 側の dependency switch 条件を明記する
- [ ] 10.4 ユーザー承認を得てから commit / push / release に進む

## 11. Definition Of Done

- [ ] 11.1 `katana-render-runtime v0.3.0` が crates.io に publish 済みである
- [ ] 11.2 `katana-diagram-renderer v0.3.0` が互換 wrapper として crates.io に publish 済みである
- [x] 11.3 MathJax runtime が TeX input を SVG output へ変換する focused test が通っている
- [x] 11.4 MathJax runtime が失敗時に diagnostics 付き raw string を返す focused test が通っている
- [x] 11.5 Mermaid / Draw.io / PlantUML の既存 runtime contract が rename 後も維持されている
  - Draw.io は `templates-uml-sequence_1` の 98.79 差分を既知差分として扱う。
- [x] 11.6 README のタイトル、説明、ロゴ、アイコン、badges、install snippet、migration note が `katana-render-runtime` 前提で更新済みである
- [ ] 11.7 KDV 側に dependency switch 可能な release として handoff 済みである
