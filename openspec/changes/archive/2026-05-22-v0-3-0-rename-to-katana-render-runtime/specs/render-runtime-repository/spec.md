## ADDED Requirements

### Requirement: Repository は katana-render-runtime へ rename されなければならない

システムは、現 repository を `katana-render-runtime` へ rename しなければならない（MUST）。新規 repository への単純コピーで履歴を断ち切ってはならない（MUST NOT）。

#### Scenario: GitHub repository を rename する

- **WHEN** repository rename を実行する
- **THEN** GitHub repository name は `katana-render-runtime` になる
- **THEN** git history は維持される
- **THEN** 旧 `katana-diagram-renderer` からの移行案内が README に存在する

### Requirement: 新 crate は v0.3.0 から公開されなければならない

システムは、`katana-render-runtime` を KDR v0.2.0 の後継として `v0.3.0` から公開しなければならない（MUST）。rename後の新 crate を `v0.1.0` として公開してはならない（MUST NOT）。

#### Scenario: 新 crate を publish する

- **WHEN** `katana-render-runtime` を crates.io に公開する
- **THEN** 初回公開 version は `0.3.0` である
- **THEN** package metadata は repository と docs の新 URL を指す
- **THEN** README は新 crate 名と責務を説明する

### Requirement: 旧 crate は互換 wrapper として残さなければならない

システムは、`katana-diagram-renderer v0.3.0` を `katana-render-runtime v0.3.0` の互換 wrapper として公開しなければならない（MUST）。旧 crate に新規実装の正本を置いてはならない（MUST NOT）。

#### Scenario: 旧 crate consumer が v0.3.0 を使う

- **WHEN** consumer が `katana-diagram-renderer = \"0.3\"` を利用する
- **THEN** 旧 crate は `katana-render-runtime` の公開 API を re-export する
- **THEN** docs には新 crate への移行案内がある
- **THEN** 新機能の実装本体は `katana-render-runtime` 側にある

### Requirement: README のロゴと公開表示は新名へ更新されなければならない

システムは、README のタイトル、説明、ロゴ、アイコン、badges、install snippet を `katana-render-runtime` 向けに更新しなければならない（MUST）。

#### Scenario: crates.io と GitHub で README を見る

- **WHEN** 利用者が README を見る
- **THEN** 先頭の名前は `katana-render-runtime` である
- **THEN** ロゴまたはアイコンは旧 diagram 専用の表示ではない
- **THEN** badges は新 crate 名と docs.rs を指す
- **THEN** 旧 `katana-diagram-renderer` は互換 wrapper として説明される

### Requirement: KDV は新 crate 公開まで待機しなければならない

KDV は、`katana-render-runtime` が crates.io に公開されるまで dependency switch を行ってはならない（MUST NOT）。

#### Scenario: KDV が dependency を切り替える

- **GIVEN** `katana-render-runtime v0.3.0` が crates.io に公開済みである
- **WHEN** KDV が dependency を更新する
- **THEN** KDV は `katana-diagram-renderer` ではなく `katana-render-runtime` を参照する
- **THEN** KDV は旧 KDR 互換 wrapper に依存しない

### Requirement: Change完了条件は v0.3.0 release まで含めなければならない

システムは、この change の完了条件に `katana-render-runtime v0.3.0` と `katana-diagram-renderer v0.3.0` wrapper の crates.io 公開を含めなければならない（MUST）。renameや実装だけで完了扱いしてはならない（MUST NOT）。

#### Scenario: Change を完了判定する

- **WHEN** `v0-3-0-rename-to-katana-render-runtime` の完了可否を判断する
- **THEN** `katana-render-runtime v0.3.0` が crates.io に公開済みである
- **THEN** `katana-diagram-renderer v0.3.0` wrapper が crates.io に公開済みである
- **THEN** README と crate metadata は `katana-render-runtime` を正本名としている
- **THEN** KDV 側へ dependency switch 可能な release として handoff されている
