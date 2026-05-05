# Tasks: katana-canvas-forge v0.1.4 viewer e2e harness

## Branch Rule

- **標準ブランチ**: `release/v0.1.4`
- **作業ブランチ**: `feature/v0.1.4-task-x`

---

## 1. Scope Baseline

### 目的

v0.1.4 を実表示 E2E 専用の変更として固定し、core library / CLI に画面表示依存を混ぜない。

### タスク

- [ ] 1.1 v0.1.0 transfer と v0.1.2 runtime asset version pinning が前提であることを確認する
- [ ] 1.2 viewer e2e の対象 artifact を SVG / PNG / JPEG / PDF / HTML として固定する
- [ ] 1.3 ImageMagick score を正本のまま維持することを確認する
- [ ] 1.4 `crates/` 配下に `floem` / `egui` を入れない制約を確認する

### Definition of Done

- [ ] v0.1.4 の目的と非目的が artifact に残っている
- [ ] viewer e2e が score の代替ではないことが明確である

---

## 2. Placement And Dependency Isolation

### 目的

実表示 E2E を `test/e2e/viewer/` に隔離する。

### タスク

- [ ] 2.1 `test/e2e/viewer/` の構成を決める
- [ ] 2.2 viewer e2e の `Cargo.toml` を workspace 非参加にする
- [ ] 2.3 viewer e2e の生成物を `tmp/viewer-e2e/` に集約する
- [ ] 2.4 `cargo tree --workspace -e normal` に `floem` / `egui` が入らない検証を追加する

### Definition of Done

- [ ] `crates/katana-canvas-forge` に画面表示依存がない
- [ ] `crates/katana-canvas-forge-cli` に画面表示依存がない
- [ ] viewer e2e は publish package に含まれない

---

## 3. Viewer E2E Runner

### 目的

reference と kcf 出力を実ウィンドウで左右比較できる viewer を作る。

### タスク

- [ ] 3.1 case 定義 format を決める
- [ ] 3.2 reference artifact と kcf artifact の読み込み処理を実装する
- [ ] 3.3 左右比較画面を実装する
- [ ] 3.4 case 切替、拡大縮小、背景色切替を実装する
- [ ] 3.5 ファイル名、寸法、score report path を画面下部に表示する
- [ ] 3.6 スクリーンショット保存を実装する

### Definition of Done

- [ ] Mermaid の最小 case を表示できる
- [ ] Draw.io の最小 case を表示できる
- [ ] export artifact の最小 case を表示できる
- [ ] 表示領域が空でないことを自動確認できる

---

## 4. Just Recipe And Local Workflow

### 目的

開発者が迷わず viewer e2e を起動できるようにする。

### タスク

- [ ] 4.1 `just viewer-e2e-open case=<name>` を追加する
- [ ] 4.2 `just viewer-e2e-screenshot case=<name>` を追加する
- [ ] 4.3 `just viewer-e2e-smoke` を追加する
- [ ] 4.4 `test/e2e/viewer/README.md` に実行方法と目視観点を書く
- [ ] 4.5 生成物が git status に混ざらないことを確認する

### Definition of Done

- [ ] local で実ウィンドウを開ける
- [ ] local でスクリーンショットを保存できる
- [ ] smoke が非0終了を検知できる

---

## 5. CI Strategy

### 目的

viewer e2e を CI に接続する範囲を固定する。

### タスク

- [ ] 5.1 Linux CI で native window smoke が安定するか確認する
- [ ] 5.2 安定する場合は `viewer-e2e-smoke` job を追加する
- [ ] 5.3 安定しない場合は CI 必須化を後続 task に分離する
- [ ] 5.4 スクリーンショットを CI artifact として保存するか決める

### Definition of Done

- [ ] CI 必須化する範囲が明確である
- [ ] 不安定な native window 起動を release gate に入れていない

---

## 6. Manual Review Boundary

### 目的

自動検証と手動目視確認の責務を分ける。

### タスク

- [ ] 6.1 自動 smoke の判定項目を固定する
- [ ] 6.2 手動目視確認の checklist を作る
- [ ] 6.3 score report と viewer screenshot の関係を README に記録する
- [ ] 6.4 viewer e2e が採点しないことを仕様に残す

### Definition of Done

- [ ] 文字欠け、ラベル切れ、線の重なり、余白、背景色が手動確認項目になっている
- [ ] ImageMagick score が正本として残っている

---

## 7. Final Verification

- [ ] 7.1 `just check` を実行する
- [ ] 7.2 `just viewer-e2e-smoke` を実行する
- [ ] 7.3 `cargo tree --workspace -e normal` に `floem` / `egui` が含まれないことを確認する
- [ ] 7.4 `/self-review` を実行する
- [ ] 7.5 `npx -y @fission-ai/openspec validate "v0-1-4-viewer-e2e-harness" --strict` を実行する
- [ ] 7.6 PR 作成が必要な場合は `/create_pull_request` を使う
