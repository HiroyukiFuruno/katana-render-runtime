# drawio-text-fallback-rendering Specification

## Purpose
TBD - created by archiving change v0-1-3-drawio-linebreak-fallback. Update Purpose after archive.
## Requirements
### Requirement: Draw.io plain text label の改行を SVG fallback で保持しなければならない

システムは、Draw.io source の plain text label（素の文字ラベル）に含まれる改行を、`foreignObject` と同じ `<switch>` 内の SVG fallback text でも複数行として保持しなければならない（MUST）。`&#10;` や実改行を、fallback `<text>` の単一 text node へ空白結合してはならない（MUST NOT）。

#### Scenario: `&#10;` の改行を fallback `<tspan>` として出力する

- **GIVEN** Draw.io cell の `value` が `First line&#10;Second line` である
- **GIVEN** 対応する fallback `<text>` に既存 `<tspan>` が無い
- **WHEN** `DrawioRenderer` が SVG を生成する
- **THEN** fallback `<text>` は `First line` と `Second line` を別々の `<tspan>` として持つ
- **THEN** fallback `<text>` は `First line Second line` の1行 text node にならない

#### Scenario: KatanA 相当の fallback 経路でも改行が残る

- **GIVEN** KatanA 側の画像化経路が `foreignObject` を描画に使わず、同じ `<switch>` 内の fallback `<text>` を使う
- **WHEN** KDR が生成した SVG を画像化する
- **THEN** 画面上では `First line` と `Second line` が2行として読める
- **THEN** KatanA 側の preprocess 変更を要求しない

### Requirement: 既存の rich text fallback merge を壊してはならない

システムは、fallback `<text>` に既に `<tspan>` がある rich text label（装飾付き文字ラベル）の行割り当てを維持しなければならない（MUST）。今回の plain text fallback 修正によって、既存 `<tspan>` の textContent 更新動作を別仕様へ広げてはならない（MUST NOT）。

#### Scenario: 既存 `<tspan>` がある fallback は既存の割り当て規則を使う

- **GIVEN** fallback `<text>` が既に複数の `<tspan>` を持つ
- **WHEN** HTML label fallback を正規化する
- **THEN** 既存 `<tspan>` の個数に合わせて label 行を割り当てる
- **THEN** `<tspan>` の作り直しを強制しない
