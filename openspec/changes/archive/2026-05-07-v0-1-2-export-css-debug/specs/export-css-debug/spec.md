> Status: 要件変更により破棄済み。active spec からは削除し、`v0.1.2` は Mermaid ZenUML / unsupported fixture handling に再割当する。

## ADDED Requirements

### Requirement: native PDF / PNG / JPEG export は HTML の body 向けCSSを反映しなければならない

システムは、KDV移譲まで維持するnative PDF / PNG / JPEG export で、HTML 内の `body` を対象にした背景色と文字色を反映しなければならない（MUST）。

#### Scenario: body の background-color を反映する

- **WHEN** HTML に `body { background-color: #1e1e1e; color: #e0e0e0; }` がある
- **THEN** PNG export の背景は暗色になる
- **THEN** JPEG export の背景は暗色になる
- **THEN** PDF export に埋め込まれる画像の背景は暗色になる

#### Scenario: selector list と background 省略指定を反映する

- **WHEN** HTML に `html, body { background: #1e1e1e; color: #e0e0e0; }` がある
- **THEN** native export は selector list から `body` を検出する
- **THEN** native export は `background` から背景色を読み取る
- **THEN** PNG / JPEG / PDF の背景は白に戻らない

#### Scenario: body 以外のセレクタを誤検出しない

- **WHEN** HTML に `tbody { color: red; }` がある
- **THEN** native export はこれを `body` のCSSとして扱わない

### Requirement: export 4形式を macOS の既定アプリで開くデバッグコマンドを提供しなければならない

システムは、1つの入力HTMLから HTML / PDF / PNG / JPG を生成し、それぞれ macOS の既定アプリで開くデバッグコマンドを提供しなければならない（MUST）。

#### Scenario: export-debug を実行する

- **WHEN** 開発者が `kcf export-debug --input sample.html` を実行する
- **THEN** `/tmp/kcf-export-debug-<pid>.html` が生成される
- **THEN** `/tmp/kcf-export-debug-<pid>.pdf` が生成される
- **THEN** `/tmp/kcf-export-debug-<pid>.png` が生成される
- **THEN** `/tmp/kcf-export-debug-<pid>.jpg` が生成される
- **THEN** HTML / PDF / PNG / JPG の順に macOS の `open` が呼ばれる

#### Scenario: テストで opener を差し替える

- **WHEN** 自動テストで `export-debug` を検証する
- **THEN** 実際のアプリを開かない
- **THEN** fake opener が4回呼ばれたことを確認する
- **THEN** 4形式の出力先が `/tmp` であることを確認する

### Requirement: export-debug は macOS 専用でなければならない

システムは、v0.1.2 の `export-debug` を macOS 専用として扱わなければならない（MUST）。

#### Scenario: OS起動方法を固定する

- **WHEN** `export-debug` が出力ファイルを開く
- **THEN** macOS の `open` を使う
- **THEN** Windows の `start` や Linux の `xdg-open` 分岐を追加しない

### Requirement: KCF export maintenance must remain migration-bound

KCF SHALL treat this export maintenance as temporary compatibility work until KDV provides equivalent export behavior.

#### Scenario: KDV export becomes available

- **WHEN** KDV provides equivalent HTML/PDF/PNG/JPG export
- **THEN** KCF export maintenance is moved or deleted
- **THEN** KCF continues to own external rendering, runtime assets, references, and score comparison
