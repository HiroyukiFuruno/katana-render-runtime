# Tasks: katana-canvas-forge v0.1.2 Mermaid ZenUML rendering

## Branch Rule

- **標準ブランチ**: `release/v0.1.2`
- **作業ブランチ**: `feature/v0.1.2-task-x`

---

## User Feedback

- [/] 未対応扱いを完了条件にしない。ZenUML は対応JSを取り込み、描画に使うことを v0.1.2 の完了条件にする。
- [/] コードブロックの構文木（AST）から種類を判定し、`zenuml` なら取り込んだJSを使う実装方針にする。
- [/] latest が v0.1.1 のため、この release change を v0.1.2 に調整する。
- [/] export-css-debug は要件変更により不要になったため、active spec から外す。
- [/] PR review の P1 指摘に対応し、CI の Playwright 依存を明示的に setup する。
- [/] PR review の directive 前置き ZenUML 判定指摘は release 主目的を壊すため P1相当として扱い、対応する。

---

## 1. ZenUML 判定

### 目的

コードブロックの構文木（AST）から Mermaid ブロックを受け取り、source 先頭の diagram type が `zenuml` かを判定する。

### タスク

- [x] 1.1 `RenderInput` / `DiagramBlock` 生成経路で Mermaid ブロックの source を保持している箇所を特定する
- [x] 1.2 `zenuml` 判定を専用型または専用関数として実装する
- [x] 1.3 frontmatter、空行、前後空白を含む判定テストを追加する
- [x] 1.4 `tests/fixtures/mermaid/en/28-zen-uml.md` と `tests/fixtures/mermaid/ja/28-zen-uml.md` が ZenUML 判定になることをテストする

### Definition of Done

- [x] `zenuml` が明示的に判定され、他の Mermaid diagram は既存経路のまま描画される

---

## 2. ZenUML 対応JS asset

### 目的

ZenUML 対応JSを repository 管理の固定 runtime asset として取り込む。

### タスク

- [x] 2.1 `@mermaid-js/mermaid-zenuml` 由来の browser 実行可能なJSを選定する
- [x] 2.2 `crates/katana-canvas-forge/vendor/mermaid-zenuml/<version>/mermaid-zenuml.min.js` に固定配置する
- [x] 2.3 `scripts/runtime-assets/runtime-asset-common.ts` に `mermaid-zenuml` kind、version、checksum、download source を追加する
- [x] 2.4 `RuntimeAsset::mermaid_zenuml()` と materialize test を追加する
- [x] 2.5 `Justfile` に `MERMAID_ZENUML_JS_VERSION` と `MERMAID_ZENUML_JS` を追加する

### Definition of Done

- [x] ZenUML 対応JSが version / checksum 付きで再取得・検証できる
- [x] runtime asset の暗黙外部依存がない

---

## 3. Mermaid runtime 登録

### 目的

ZenUML 判定時に、取り込んだJSを Mermaid runtime に登録してから描画する。

### タスク

- [x] 3.1 `MermaidRuntimeScripts::source_scripts()` に `mermaid-zenuml.min.js` と bridge script を追加する
- [x] 3.2 読み込み順を `mermaid.min.js` → `mermaid-zenuml.min.js` → bridge → `render-mermaid.js` に固定する
- [x] 3.3 bridge script で ZenUML module を `globalThis.__katanaMermaidZenuml` に正規化する
- [x] 3.4 `render_mermaid.js` で `zenuml` 判定時に `mermaidValue.registerExternalDiagrams([globalThis.__katanaMermaidZenuml])` を render 前に実行する
- [x] 3.5 `globalThis.__katanaMermaidZenuml` が存在しない場合は fallback せず error にする
- [x] 3.6 fake runtime test で `registerExternalDiagrams` が render 前に呼ばれることを確認する

### Definition of Done

- [x] `28-zen-uml.md` が `UnknownDiagramError` にならず SVG を返す
- [x] fallback SVG / stub PNG を使っていない

---

## 4. 公式参照と比較

### 目的

公式参照生成と Katana runtime の両方で ZenUML を描画し、score 対象として比較する。

### タスク

- [x] 4.1 `scripts/mermaid/official-renderer.ts` に ZenUML 対応JS読み込みと external diagram 登録を追加する
- [x] 4.2 `diagram-update` で `en/28-zen-uml.md` と `ja/28-zen-uml.md` の参照SVG / PNGを更新する
- [x] 4.3 `mermaid-compare-full` で `28-zen-uml.md` が score 対象に含まれることを確認する
- [x] 4.4 参照更新差分に fallback SVG / stub PNG が含まれていないことを確認する

### Definition of Done

- [x] `28-zen-uml.md` が通常 fixture として比較される
- [x] ZenUML の比較結果が report に出る

---

## 5. Final Verification

- [x] 5.1 `just mermaid-compare-full` を実行する
- [x] 5.2 `just check` を実行する
- [x] 5.3 `git diff --check` を実行する
- [ ] 5.4 PR 作成後に `@codex review` を依頼する
- [x] 5.5 `npx -y @fission-ai/openspec validate "v0-1-2-mermaid-zenuml-fixture-support" --strict` を実行する
- [x] 5.6 `just VERSION=v0.1.2 release-check` を実行する
