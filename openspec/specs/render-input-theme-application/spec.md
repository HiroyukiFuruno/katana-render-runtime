# render-input-theme-application Specification

## Purpose
外部 consumer が `RenderInput` 経由で渡した theme snapshot を、kdr の既定 preset より優先して runtime request と cache fingerprint に反映するための仕様を定義する。
## Requirements
### Requirement: RenderInput の theme 情報で既定 preset を上書きできなければならない

システムは、`RenderInput.context.theme` が `Some` の場合、その snapshot から組み立てた `DiagramColorPreset` を Mermaid / Draw.io の runtime request に渡さなければならない（MUST）。これは既存の描画アルゴリズムを変えるものではなく、外部指定値で既定 preset を上書きする経路でなければならない（MUST）。`DiagramColorPreset::current()` などの process global state を、snapshot 指定時に参照してはならない（MUST NOT）。

#### Scenario: light snapshot を渡したとき global が dark でも light preset が使われる

- **GIVEN** `DiagramColorPreset` の global state（`DARK_MODE`）が true
- **GIVEN** `RenderInput.context.theme = Some(light snapshot)` の入力
- **WHEN** `MermaidRenderer::render()` または `DrawioRenderer::render()` を呼ぶ
- **THEN** 描画層に渡される `DiagramColorPreset` は light snapshot 由来の値である
- **THEN** Mermaid / Draw.io の runtime request は light snapshot 由来の値を持つ

#### Scenario: snapshot が None のとき global state を fallback として使う

- **GIVEN** `RenderInput.context.theme = None` の入力
- **WHEN** renderer を呼ぶ
- **THEN** 描画層に渡される preset は `DiagramColorPreset::current()` の値を使う
- **THEN** 既存 consumer / 内部 script の挙動は変わらない

### Requirement: cache fingerprint は RenderInput 由来の実効テーマで変化しなければならない

システムは、`CacheFingerprintOps::render(input, ..)` の結果を、`RenderInput.context.theme` の差分で必ず変化させなければならない（MUST）。snapshot が `Some` のとき、global state の差分で fingerprint を変化させてはならない（MUST NOT）。

#### Scenario: theme snapshot 差分で fingerprint が変わる

- **GIVEN** 同一 source で `RenderInput.context.theme` が light / dark に異なる 2 入力
- **WHEN** 各々 `CacheFingerprintOps::render()` を呼ぶ
- **THEN** 2 つの fingerprint は異なる
- **THEN** runtime version / checksum が同じでも fingerprint は異なる

#### Scenario: fallback path でも global state 差分で fingerprint が変わる

- **GIVEN** `RenderInput.context.theme = None` の入力
- **WHEN** `DiagramColorPreset` の global state を切り替えて 2 回 `CacheFingerprintOps::render()` を呼ぶ
- **THEN** 2 つの fingerprint は異なる

### Requirement: 公開 DTO に typed theme snapshot を提供しなければならない

システムは、`RenderContext` に typed な `RenderThemeSnapshot` を任意 field として提供し、consumer が theme 情報を type-safe に渡せる経路を用意しなければならない（MUST）。`RenderThemeSnapshot` は `Hash` / `Clone` / `Serialize` / `Deserialize` を満たさなければならない（MUST）。

#### Scenario: RenderContext に theme snapshot を渡す

- **GIVEN** consumer が `RenderThemeSnapshot { mode: Light, .. }` を持つ `RenderContext` を構築する
- **WHEN** `RenderInput` を経由して renderer を呼ぶ
- **THEN** snapshot 由来の値が runtime request と cache fingerprint に使われる
- **THEN** `RenderContext::default()` は `theme: None` を返し、既存 consumer の build を壊さない

### Requirement: runtime request は外部指定 theme の値を保持しなければならない

システムは、同一 source かどうかに関係なく、`RenderInput.context.theme` から組み立てた preset 値を runtime request へ渡さなければならない（MUST）。この検証は request / preset の値確認で行い、score 評価や画像類似度の再評価を必要としてはならない（MUST NOT）。

#### Scenario: Mermaid request が preset 由来の値を持つ

- **GIVEN** light の `RenderThemeSnapshot` から組み立てた `DiagramColorPreset`
- **WHEN** Mermaid の runtime request を作る
- **THEN** `theme` / `background` / `fill` / `text` / `stroke` / `arrow` は preset 由来の値である

#### Scenario: Draw.io request が preset 由来の値を持つ

- **GIVEN** light の `RenderThemeSnapshot` から組み立てた `DiagramColorPreset`
- **WHEN** Draw.io の runtime request を作る
- **THEN** `dark_mode` / `background` は preset 由来の値である

### Requirement: release PR 前に対象 OpenSpec change を archive 済みにしなければならない

システムは、`release/v...` branch から取り込み依頼（Pull Request）を作成する前に、対象 version 以前の active OpenSpec change が `openspec/changes/` に残っていないことを検査しなければならない（MUST）。

#### Scenario: release branch の pre-pr で archive 漏れを検出する

- **GIVEN** 現在の branch が `release/v0.1.3`
- **GIVEN** `openspec/changes/v0-1-2-mermaid-zenuml-fixture-support` または `openspec/changes/v0-1-3-render-input-theme-application` が active 側に残っている
- **WHEN** `lefthook run pre-pr` を実行する
- **THEN** 検査は失敗する
- **THEN** 利用者には対象 change を archive へ移動する必要があることが示される

#### Scenario: OpenSpec を持たない repository では archive 確認を通す

- **GIVEN** `openspec/changes/` が存在しない repository
- **WHEN** release branch の pre-pr archive gate を実行する
- **THEN** 検査は成功し、archive 確認はスキップされる

### Requirement: CI は PR / master merge / master direct push を分岐しなければならない

システムは、取り込み依頼（Pull Request）では full check を必ず実行し、`master` への merge push では Rust full check を再実行してはならない（MUST NOT）。`master` への直接 push では、動作影響がある差分だけ Rust full check を実行しなければならない（MUST）。

#### Scenario: PR は文書差分でも full check を実行する

- **GIVEN** `master` 向けの取り込み依頼（Pull Request）
- **WHEN** 変更内容が `*.md` だけである
- **THEN** CI は Rust full check を実行する
- **THEN** required check が通らない限り merge できない

#### Scenario: release PR merge 後の master push は Rust full check を省く

- **GIVEN** `release/v...` の取り込み依頼（Pull Request）が `master` へ merge された
- **WHEN** merge commit による `push` event が発生する
- **THEN** CI は Rust full check を省く
- **THEN** Release workflow が release 処理を担当する

#### Scenario: master 直接 push は動作影響差分だけ Rust full check を実行する

- **GIVEN** `master` へ直接 push された
- **WHEN** 差分が `*.md`、`scripts/`、`.github/`、`openspec/`、`.agents/`、`docs/`、`assets/` だけである
- **THEN** CI は Rust full check を省く
- **WHEN** 差分に Rust crate、`Cargo.toml`、`Cargo.lock`、`Justfile` など動作影響がある file が含まれる
- **THEN** CI は Rust full check を実行する
