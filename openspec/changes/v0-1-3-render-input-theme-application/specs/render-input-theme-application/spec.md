## ADDED Requirements

### Requirement: RenderInput の theme 情報を実描画に反映しなければならない

システムは、`RenderInput.context.theme` が `Some` の場合、その snapshot から組み立てた `DiagramColorPreset` を Mermaid / Draw.io の実描画に渡さなければならない（MUST）。`DiagramColorPreset::current()` などの process global state を、snapshot 指定時に参照してはならない（MUST NOT）。

#### Scenario: light snapshot を渡したとき global が dark でも light で描画される

- **GIVEN** `DiagramColorPreset` の global state（`DARK_MODE`）が true
- **GIVEN** `RenderInput.context.theme = Some(light snapshot)` の入力
- **WHEN** `MermaidRenderer::render()` または `DrawioRenderer::render()` を呼ぶ
- **THEN** 描画層に渡される `DiagramColorPreset` は light snapshot 由来の値である
- **THEN** 出力 SVG は light 配色で描画される

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
- **THEN** snapshot 由来の値が描画と cache fingerprint に使われる
- **THEN** `RenderContext::default()` は `theme: None` を返し、既存 consumer の build を壊さない

### Requirement: 同一 source で light / dark の RenderInput は異なる出力を返さなければならない

システムは、同一 source、異なる `RenderInput.context.theme` の 2 入力について、renderer の出力 SVG または描画 request が異なる結果を返さなければならない（MUST）。

#### Scenario: light / dark snapshot で出力が分岐する

- **GIVEN** 同一 Mermaid / Draw.io source
- **GIVEN** light / dark の `RenderThemeSnapshot` をそれぞれ持つ 2 つの `RenderInput`
- **WHEN** renderer を 2 回呼ぶ
- **THEN** 出力 SVG または描画層への request が 2 つの間で異なる
- **THEN** いずれの結果も fallback SVG ではなく実描画 SVG である
