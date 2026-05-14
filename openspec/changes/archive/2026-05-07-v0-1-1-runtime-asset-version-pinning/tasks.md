# Tasks: katana-diagram-renderer v0.1.1 runtime asset version pinning

## Branch Rule

- **標準ブランチ**: `release/v0.1.1`
- **作業ブランチ**: `feature/v0.1.1-task-x`

---

## 1. Runtime Asset Inventory

### 目的

v0.1.0 transfer 後の Mermaid.js / Draw.io.js asset 管理状態を確認する。

### タスク

- [x] 1.1 Mermaid.js の取り込み元、version、checksum、参照箇所を一覧化する
- [x] 1.2 Draw.io.js の取り込み元、version、checksum、resource manifest、参照箇所を一覧化する
- [x] 1.3 runtime metadata と cache fingerprint が version / checksum を扱えるか確認する

### Definition of Done

- [x] 固定対象の asset と参照箇所が artifact に残っている

---

## 2. Version Pinning

### 目的

Mermaid.js / Draw.io.js の取り込み version を kdr 側で固定する。

### タスク

- [x] 2.1 Mermaid.js 固定 version 定数または manifest を追加する
- [x] 2.2 Draw.io.js 固定 version 定数または manifest を追加する
- [x] 2.3 checksum を runtime metadata と検証に接続する
- [x] 2.4 version 変更時に reference snapshot 更新が必要なことを検知する

### Definition of Done

- [x] version と checksum が実行時 metadata に現れる
- [x] version が不明な asset を暗黙に読み込まない

---

## 3. Latest Check Recipe

### 目的

現在固定 version と取得可能な latest version を確認できるようにする。

### タスク

- [x] 3.1 Mermaid.js latest check recipe を追加する
- [x] 3.2 Draw.io.js latest check recipe を追加する
- [x] 3.3 latest check は repository 内 file を変更しないようにする

### Definition of Done

- [x] latest check の出力に current / latest / update hint が含まれる
- [x] latest check 実行後に `git status --short` が変化しない

---

## 4. Update Recipe

### 目的

指定 version を取り込み、checksum、manifest、full / representative の reference snapshot、compare を一括実行する。

### タスク

- [x] 4.1 Mermaid.js update recipe を追加する
- [x] 4.2 Draw.io.js update recipe を追加する
- [x] 4.3 update recipe が checksum を更新する
- [x] 4.4 update recipe が full / representative の reference snapshot を再生成する
- [x] 4.5 update recipe が local full compare と CI/CD representative compare を実行し、score 低下を検知する

### Definition of Done

- [x] 指定 version 取り込みが再現可能である
- [x] render script、resource manifest、checksum、reference snapshot の更新漏れを CI で検知できる

---

## 5. Final Verification

- [x] 5.1 `/lint-and-ast-lint` を実行する
- [x] 5.2 `/self-review` を実行する
- [x] 5.3 `npx -y @fission-ai/openspec validate "v0-1-1-runtime-asset-version-pinning" --strict` を実行する
- [x] 5.4 PR 作成が必要な場合は `/create_pull_request` を使う
