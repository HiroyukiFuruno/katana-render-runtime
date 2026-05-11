# Tasks: katana-canvas-forge v0.1.2 export CSS debug

> Status: 要件変更により破棄済み。`v0.1.2` は Mermaid ZenUML / unsupported fixture handling に再割当する。

## Branch Rule

- **標準ブランチ**: `release/v0.1.2`
- **作業ブランチ**: `feature/v0.1.2-task-x`

---

## 1. Scope Baseline

### 目的

v0.1.2 を export CSS 回帰修正と macOS デバッグ実行のフェーズとして固定する。

### タスク

- [x] 1.1 score improvement を v0.1.x 最終フェーズへ送り、この change を v0.1.2 に固定する
- [x] 1.2 v0.1.2 の対象を PDF / PNG / JPEG のCSS反映と export 4形式デバッグに固定する
- [x] 1.3 macOS だけをサポート対象にし、Windows / Linux 対応を入れない
- [x] 1.4 PDF と画像の出力先を `/tmp` に固定する
- [x] 1.5 このexport保守はKDV移譲までの暫定維持であり、新規export責務ではないことを明記する

### Definition of Done

- [x] v0.1.2 の目的と非目的が artifact に残っている
- [x] v0.1.4 との依存関係が明確である
- [x] KDV移譲後のKCF側export削除方針と矛盾していない

---

## 2. Regression Tests

### 目的

CSSが当たらない回帰を先にテストで固定する。

### タスク

- [x] 2.1 `html, body { background: ...; color: ... }` の export 回帰テストを追加する
- [x] 2.2 PNG / JPEG / PDF のサンプルピクセルが暗色背景になることを確認する
- [x] 2.3 既存の `body { background-color: ... }` テストに JPEG を含める

### Definition of Done

- [x] 修正前に少なくとも1件の回帰テストが失敗する
- [x] 修正後に PNG / JPEG / PDF の回帰テストが通る

---

## 3. Native CSS Fix

### 目的

native export の軽量CSS解釈で、`body` 向けCSSを正しく拾う。

### タスク

- [x] 3.1 `<style>...</style>` の中を優先してCSS規則を読む
- [x] 3.2 selector list から `body` を検出する
- [x] 3.3 `background-color` と `background` の両方から背景色を読む
- [x] 3.4 `body` 以外の `tbody` などを誤検出しない

### Definition of Done

- [x] HTML の背景色が PDF / PNG / JPEG に反映される
- [x] 既存の export テストが維持される

---

## 4. Export Debug Command

### 目的

1つの入力HTMLから export 4形式を生成し、macOS の既定アプリで開けるようにする。

### タスク

- [x] 4.1 `kcf export-debug --input <html>` を追加する
- [x] 4.2 `/tmp/kcf-export-debug-<pid>.html` を出力する
- [x] 4.3 `/tmp/kcf-export-debug-<pid>.pdf` を出力する
- [x] 4.4 `/tmp/kcf-export-debug-<pid>.png` を出力する
- [x] 4.5 `/tmp/kcf-export-debug-<pid>.jpg` を出力する
- [x] 4.6 HTML / PDF / PNG / JPG の順に macOS の `open` を呼ぶ
- [x] 4.7 テストでは実際のアプリを開かず、opener を差し替えて検証する

### Definition of Done

- [x] CLI が `export-debug` を parse できる
- [x] 4形式のファイルが `/tmp` に作られる
- [x] opener が4回呼ばれる

---

## 5. Final Verification

- [x] 5.1 `cargo test -p katana-canvas-forge --test exporter_visual_transfer` を実行する
- [x] 5.2 `cargo test -p katana-canvas-forge native_style -- --nocapture` を実行する
- [x] 5.3 `cargo test -p katana-canvas-forge-cli export_debug -- --nocapture` を実行する
- [x] 5.4 `cargo test -p katana-canvas-forge-cli cli_export_debug_writes_and_opens_four_formats -- --nocapture` を実行する
- [x] 5.5 `just lint` と `just ast-lint` を実行する
- [x] 5.6 `npx -y @fission-ai/openspec validate "export-css-debug" --strict` を実行する
- [x] 5.7 `/self-review` を実行する
