# Tasks: katana-canvas-forge v0.1.0 transfer

## 実施状況メモ（2026-05-05）

- [x] kcf 側への Mermaid / Draw.io / export / score 移植は release branch 上で実装済み
- [x] PR #1 は close 済みで、差分は merge / cherry-pick していない
- [x] `just check`、`just VERSION=v0.1.0 release-check`、`git diff --check`、OpenSpec strict validate は coverage 100% gate へ更新後に再実行済み
- [x] KatanA 由来の Mermaid / Draw.io / export 検証観点は kcf 側 integration test と reference compare に移管済み
- [x] `just coverage` は行カバレッジ（line coverage）100%、未到達行（uncovered line）0 で通過済み
- [x] Mermaid score smoke は 1 fixture を比較し、minimum score 99.83 を確認済み
- [x] Draw.io score smoke は 1 fixture を比較し、minimum score 99.99 を確認済み
- [x] Mermaid representative compare は 10 fixture を比較し、既存 score floor 付きで通過済み（minimum score 88.85）
- [x] Draw.io representative compare は 20 fixture を比較し、`score-baseline.json` 付きで通過済み（minimum score 93.20）
- [/] v0.1.0 は KatanA 側へまだ取り込まないため release し、full score 未達は v0.1.4 reference score improvement に送る方針を記録済み
- [/] v0.1.x 後続要件として、v0.1.1 に runtime asset version pinning、v0.1.2 に Mermaid ZenUML / unsupported fixture handling、v0.1.3 に RenderInput theme application、v0.1.4 に reference score improvement、v0.1.5 にKDV export handoffを置く方針を design.md に記録済み
- [/] 公式 reference SVG / PNG は git 管理し、CI/CD では再取得・再生成しない方針を proposal / design / spec に記録済み
- [/] ローカルは全量評価、CI/CD は代表ケース評価、`basic` は疎通確認専用とする方針を proposal / design / spec に記録済み
- [/] Justfile の `kcf-*` recipe 名は v0.1.0 公開前品質として整理済み
- [/] README の `Scaffolding` と古い KatanA change 参照は v0.1.0 公開前品質として整理済み
- [/] `.serena/project.yml` の差分は v0.1.0 release 成果に含めず、commit 対象から除外する
- [x] commit / push は完了済み。master 直接作業として完了しているため、PR 作成 / cloud review は不要である
- [/] KatanA 側で kcf v0.1.0 tag を consume する確認は KatanA 側の関心に分離し、kcf v0.1.0 の完了条件から外す

## Branch Rule

- **標準ブランチ**: `release/v0.1.0`
- **作業ブランチ**: `feature/v0.1.0-task-x`

---

## 0. Planning Baseline

### 目的

v0.1.0 を KatanA 既存実装の忠実移植として固定し、PR #1 の簡略実装へ戻らないようにする。

### 書き込み範囲

- `openspec/`
- GitHub PR #1 の参照履歴

### タスク

- [x] 0.1 PR #1 を close し、実装差分は土台にしない方針を明確化する
- [x] 0.2 PR #1 のレビュー指摘を、v0.1.0 実装時の失敗パターンとして確認する
- [x] 0.3 v0.1.0 の完了条件を transfer として再確認する

### Definition of Done

- [x] PR #1 の差分を merge / cherry-pick しないことが確認されている
- [x] v0.1.0 が Mermaid / Draw.io / export / score の移植であることが明文化されている
- [x] extension 要件が v0.1.0 から分離されている

---

## 1. 移植元棚卸しと責務境界を固定する

### 目的

KatanA 側の移植元を漏れなく洗い出し、kcf 側の crate / module / CLI / vendor 配置へ写像する。

### 書き込み範囲

- `openspec/`
- `crates/katana-canvas-forge/src/`
- `crates/katana-canvas-forge-cli/src/`
- `vendor/`
- `tests/`

### タスク

- [x] 1.1 `crates/katana-core/src/markdown/mermaid_renderer/` の移植対象を一覧化する
- [x] 1.2 `crates/katana-core/src/markdown/drawio_renderer/` と resource 一式の移植対象を一覧化する
- [x] 1.3 `crates/katana-core/src/markdown/export/` の HTML / PDF / PNG / JPEG export 対象を一覧化する
- [x] 1.4 `scripts/mermaid/` と `scripts/drawio/` の reference 生成・採点対象を一覧化する
- [x] 1.5 `assets/fixtures/mermaid_parts/` と `assets/fixtures/drawio/` の移植方針を決める
- [x] 1.6 KatanA 側で定義済みの renderer / exporter interface と DTO を一覧化する
- [x] 1.7 KatanA UI state、preview state、workspace state を kcf に持ち込まない境界を確認する

### Definition of Done

- [x] 移植対象と非移植対象が artifact に残っている
- [x] KatanA 固有依存を剥がす箇所が明確である
- [x] v0.1.0 の作業範囲に extension 要件が混ざっていない

---

## 2. kcf 公開 API で KatanA 既存能力を維持する

### 目的

KatanA 側でできていた Mermaid / Draw.io / export / score の能力を落とさない kcf 公開 API を固定する。KatanA 側の型名完全一致は必須にせず、薄い adapter で接続できる情報量を守る。

### 書き込み範囲

- `crates/katana-canvas-forge/src/renderer.rs`
- `crates/katana-canvas-forge/src/exporter.rs`
- `crates/katana-canvas-forge/src/lib.rs`
- `tests/`

### タスク

- [x] 2.1 KatanA 側の `Renderer` 相当能力を kcf の `Renderer` / `RenderInput` / `RenderOutput` / `RenderError` で表現する
- [x] 2.2 KatanA 側の `Exporter` 相当能力を kcf の `ExporterTrait` / `ExportInput` / `ExportOutput` / `ExportError` で表現する
- [x] 2.3 KatanA 側の `diagram_backend` DTO と kcf 公開 API の対応表を design / test に反映する
- [x] 2.4 Mermaid / Draw.io / export の失敗経路を KatanA 既存挙動と同等に表現する
- [x] 2.5 KatanA consumer adapter で必要な情報が DTO にそろっていることを kcf 側の回帰テストで確認する
- [x] 2.6 trait 境界を実装経由で通す回帰テストを追加する

### Definition of Done

- [x] 公開 API が KatanA 側の既存能力を落とさない
- [x] kcf 独自都合で field や format を削った結果、KatanA 側で機能差分の再実装が必要になっていない
- [x] 公開 API が KatanA UI 固有型を含まない
- [x] `cargo tree --workspace -e normal` に `egui` と KatanA UI state が含まれない
- [x] CLI が DTO を再定義していない

---

## 3. Mermaid backend を移植する

### Definition of Ready

- [x] Task 1 と Task 2 が完了している
- [x] Mermaid runtime asset を v0.1.0 でどう移すかが決まっている

### 目的

KatanA 既存 Mermaid runtime と fixture coverage を kcf へ移す。

### 書き込み範囲

- `crates/katana-canvas-forge/src/mermaid/`
- `vendor/mermaid/`
- `tests/fixtures/mermaid/`
- `tests/`

### タスク

- [x] 3.1 KatanA Mermaid runtime script、DOM shim、normalizer、theme、i18n 処理を移植する
- [x] 3.2 Mermaid runtime asset を KatanA 相当の読み込み形態で移す
- [x] 3.3 Mermaid fixture と snapshot を KatanA から移す
- [x] 3.4 `RenderOutput` の SVG、width、height、viewBox、runtime、diagnostics、cache fingerprint を実出力から生成する
- [x] 3.5 Mermaid backend の KatanA 由来検証観点を kcf の unit test / integration test / reference compare に移す

### Definition of Done

- [x] PR #1 の簡略 SVG 生成ではなく、KatanA 既存 runtime と同等の出力である
- [x] version 固定と更新 recipe 改善は v0.1.1 に送られている
- [x] Mermaid 既存 fixture と KatanA 由来 runtime 検証観点が kcf 側で検証されている

---

## 4. Draw.io backend と resource 一式を移植する

### Definition of Ready

- [x] Task 1 と Task 2 が完了している
- [x] Draw.io runtime asset と resource を v0.1.0 でどう移すかが決まっている

### 目的

KatanA 既存 Draw.io runtime と resource resolver を kcf へ移す。

### 書き込み範囲

- `crates/katana-canvas-forge/src/drawio/`
- `vendor/drawio/`
- `tests/fixtures/drawio/`
- `tests/`

### タスク

- [x] 4.1 Draw.io runtime adapter、mx adapter、SVG postprocess、resource resolver を移植する
- [x] 4.2 Draw.io resource、stencil、image、manifest を kcf 管理へ移す
- [x] 4.3 Draw.io runtime asset、resource、manifest を KatanA 相当の読み込み形態で移す
- [x] 4.4 Draw.io fixture と official reference を KatanA から移す
- [x] 4.5 Draw.io backend の KatanA 由来検証観点を kcf の unit test / integration test / reference compare に移す

### Definition of Done

- [x] KatanA 既存 Draw.io tests と同等の検証観点が kcf 側にある
- [x] resource 解決が kcf 内で完結している
- [x] KatanA UI state に依存していない

---

## 5. HTML / PDF / PNG / JPEG export を移植する

### Definition of Ready

- [x] Task 2 が完了している
- [x] export input が HTML source と rendered SVG のどちらを受けるかが設計で固定されている

### 目的

KatanA 既存 export runtime を kcf へ移し、HTML だけで完了扱いにしない。

### 書き込み範囲

- `crates/katana-canvas-forge/src/exporter/`
- `tests/`

### タスク

- [x] 5.1 HTML export と template 処理を移植する
- [x] 5.2 PDF export を移植する
- [x] 5.3 PNG export を移植する
- [x] 5.4 JPEG export を移植する
- [x] 5.5 export regression test と native visual export test の検証観点を kcf の `Exporter` 入力で移す

### Definition of Done

- [x] HTML / PDF / PNG / JPEG が `Exporter` 経由で出力できる
- [x] 未対応 format のみ `UnsupportedFormat` を返す
- [x] export は KatanA の Markdown renderer や UI state に密結合していない

---

## 6. Mermaid / Draw.io 採点評価を移植する

### Definition of Ready

- [x] Task 3 と Task 4 が完了している
- [x] ImageMagick の実行前提と CI install 方法が決まっている

### 目的

KatanA 既存の公式 reference 生成と画像 score を kcf に移す。

### 書き込み範囲

- `crates/katana-canvas-forge-cli/src/`
- `tests/reference/`
- `scripts/` または kcf CLI 内部 module
- `.github/workflows/`

### タスク

- [x] 6.1 Mermaid official reference 生成を移植し、SVG / PNG を git 管理対象にする
- [x] 6.2 Mermaid ImageMagick score と baseline policy を移植する
- [x] 6.3 Draw.io official reference 生成を移植し、SVG / PNG を git 管理対象にする
- [x] 6.4 Draw.io ImageMagick score、PHASH、dimension coverage を移植する
- [x] 6.5 contact sheet と report 生成を移植する
- [x] 6.6 SVG 文字列比較だけでは合格できない検証を追加する
- [x] 6.7 Draw.io official full fixture と SVG / PNG reference を KatanA から移植する
- [x] 6.8 Mermaid / Draw.io の CI/CD 用 representative fixture を git 管理対象にする
- [x] 6.9 Draw.io representative の既知 score 下限を baseline file として git 管理対象にする

### Definition of Done

- [x] `kcf mermaid compare --min-score <score>` が画像 score を出力する
- [x] `kcf drawio compare --min-score <score>` が画像 score を出力する
- [x] reference-update と compare が同じ fixture 管理を使う
- [x] compare は git 管理済み reference を読み、CI/CD で reference-update を実行しない
- [x] `basic` fixture は疎通確認専用であり、vendor 互換性保証の代替ではない
- [x] ローカル full compare と CI/CD representative compare の入口が分離されている
- [x] Draw.io representative は既知下限からの悪化を検知できる

---

## 7. CLI / just / CI を移植運用に接続する

### Definition of Ready

- [x] Task 3 から Task 6 が完了している

### 目的

kcf CLI を library の薄い利用者として整え、移植後の品質ゲートを自動化する。

### 書き込み範囲

- `crates/katana-canvas-forge-cli/src/`
- `Justfile`
- `.github/workflows/`
- `lefthook.yml`

### タスク

- [x] 7.1 `kcf mermaid render / reference-update / compare / bench` を接続する
- [x] 7.2 `kcf drawio render / reference-update / compare / bench` を接続する
- [x] 7.3 `kcf export html / pdf / png / jpeg` を接続する
- [x] 7.4 just recipe を render、reference-update、compare、bench、export に接続する
- [x] 7.5 local full compare と CI/CD representative compare の just recipe を分ける
- [x] 7.6 CI で fmt / clippy / test / coverage / dependency leak / release-preflight を実行する
- [/] 7.7 reference score の CI 必須化は、v0.1.4 reference score improvement と v0.1.1 runtime asset pinning の後に接続する
- [x] 7.8 lefthook または equivalent で軽量な pre-commit gate を設定する

### Definition of Done

- [x] CLI が KatanA 固有 path を前提にしない
- [x] runtime asset version 固定と更新 recipe 改善は v0.1.1 に送られている
- [x] CI が PR #1 型の簡略実装を通常 test / coverage / dependency leak で検知できる

---

## 8. KatanA adapter readiness を kcf 側で確認する

### Definition of Ready

- [x] Task 7 が完了している

### 目的

KatanA 側の実組み込みは KatanA の関心として分離し、kcf では薄い adapter で必要になる公開能力が欠けていないことを確認する。

### 書き込み範囲

- `openspec/`
- `crates/katana-canvas-forge/tests/`
- kcf 側 integration fixture

### タスク

- [x] 8.1 KatanA 側 `diagram_backend` 入出力と kcf `Render*` / `Export*` 入出力の対応を確認する
- [x] 8.2 Mermaid / Draw.io / export / score の既存能力を kcf 側テストと reference compare に寄せる
- [x] 8.3 KatanA 側実組み込みは kcf release 後の KatanA 側 task として扱うことを記録する

### Definition of Done

- [x] kcf 公開 API から KatanA adapter に必要な情報が欠落していない
- [x] KatanA 側実組み込みを kcf v0.1.0 完了条件に含めていない
- [x] KatanA 側に残したくない検証観点が kcf 側のテストまたは reference compare に移っている

---

## 9. 品質基準と nullable 境界を是正する

### 目的

移植済みロジックを 83% 台の未検証状態で merge しない。不要な fallback / dead logic は削除し、必要な失敗経路は error first として検証する。

### 書き込み範囲

- `Justfile`
- `docs/`
- `openspec/`
- `crates/katana-canvas-forge/src/renderer/`
- `crates/katana-canvas-forge/src/markdown/`
- `crates/kcf-linter/`

### タスク

- [x] 9.1 `just coverage` を行カバレッジ（line coverage）100%、未到達行（uncovered line）0 に固定する
- [x] 9.2 最表層で未指定に仕様上の意味がある入力だけ nullable を許し、内部へ入る前に非 null へ解決する方針を coding rules に明文化する
- [x] 9.3 CLI `--runtime` は `RuntimePathResolver` で `PathBuf` または `RenderError::RuntimeResolution` に変換し、renderer 内部の `Option<PathBuf>` を削除する
- [x] 9.4 暗黙 fallback、過剰な error swallowing、到達不能に近い dead logic を削除または error first の失敗経路へ変更する
- [x] 9.5 `just coverage`、`just check`、`just VERSION=v0.1.0 release-check`、OpenSpec strict validate、`git diff --check` を再実行する

### Definition of Done

- [x] coverage gate が 100% / 未到達行 0 で通過する
- [x] runtime path の nullable は CLI 境界に限定され、renderer 内部は非 null である
- [x] coverage のためだけに残した fallback / dead logic がない
- [x] OpenSpec と docs が更新後の実装方針と一致している

---

## 9. User Review

> ユーザーから受けた指摘は `[/]` で閉じる。通常の開発タスク `[x]` と混ぜない。

- [x] 9.1 実装結果と検証結果をユーザーに提示済みである
- [x] 9.2 Mermaid / Draw.io / export / score の移植完了証跡を提示済みである
- [x] 9.3 フィードバックを本 `tasks.md` に追記し、対応済みを `[/]` にする
- [/] 9.4 kcf は独立機能なので、KatanA 側 interface の型名完全一致ではなく、KatanA 側の既存能力が落ちないことを完了基準にする
- [/] 9.5 KatanA 側を削ぐため、KatanA 由来テストの検証観点を kcf 側へ寄せる

---

## 10. Final Verification

- [x] 10.1 `/lint-and-ast-lint` 相当として `just check` を実行し、静的検査と抽象構文木検査の結果を記録する
- [x] 10.2 `/self-review` を実行し、差分範囲の設計、テスト、検証の妥当性を確認する
- [x] 10.3 `npx -y @fission-ai/openspec validate "v0-1-0-renderer-interface-and-mermaid-backend" --strict` を実行する
- [x] 10.4 master 直接作業として完了しているため、PR 作成は不要である
- [x] 10.5 `/openspec-archive-change` 手順で archive へ移す
