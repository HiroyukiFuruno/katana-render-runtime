## ADDED Requirements

### Requirement: score 改善は既知下限を上げなければならない

システムは、v0.1.0 で移植した reference score の既知未達 case について、現在値を追認するのではなく、修正可能な差分を直して score 下限を上げなければならない（MUST）。

#### Scenario: supported fixture は 99 以上で合格する

- **GIVEN** Mermaid または Draw.io の supported fixture がある
- **WHEN** v0.1.2 の score 改善を完了する
- **THEN** `just drawio-compare-ci 99` が通る
- **THEN** `just drawio-compare-full 99` が通る
- **THEN** `just mermaid-compare-ci 99` が通る
- **THEN** `just mermaid-compare-full 99` が通る
- **THEN** full compare 対象の全 supported pattern に 99 点未満の case を残さない

#### Scenario: full compare を完了判定にする

- **GIVEN** CI compare と full compare がある
- **WHEN** v0.1.2 の score 改善を判定する
- **THEN** CI compare は高速な代表確認として扱う
- **THEN** full compare は全 supported pattern の完了判定として扱う
- **THEN** CI compare だけの成功で完了扱いにしない

#### Scenario: Draw.io representative baseline を更新する

- **GIVEN** `tests/fixtures/drawio/representative/score-baseline.json` に既知下限がある
- **WHEN** v0.1.2 の score 改善を行う
- **THEN** baseline は改善後の score に合わせて 99 以上へ上げる
- **THEN** baseline を下げて合格扱いにしない

#### Scenario: Mermaid accepted score floor を更新する

- **GIVEN** `scripts/mermaid/reference_score_floors.ts` に accepted score floor がある
- **WHEN** v0.1.2 の score 改善を行う
- **THEN** supported fixture の floor は 99 以上へ上げる
- **THEN** 99 未満の floor を理由に full compare を合格扱いにしない

#### Scenario: full compare の未達を調査する

- **GIVEN** Mermaid または Draw.io full compare で 99 点未満の case がある
- **WHEN** 開発者が score 改善を実施する
- **THEN** 未達原因を renderer、resource、postprocess、reference 特殊ケースに分類する
- **THEN** supported case は 99 点以上へ修正する
- **THEN** v0.1.2 で修正しない case はユーザー確認後に別 change へ送り、理由と score を report に残す

#### Scenario: unsupported fixture を明示する

- **GIVEN** v0.1.2 では修正しない unsupported fixture がある
- **WHEN** full compare の対象を整理する
- **THEN** unsupported fixture 名、理由、後続 change を report に記録する
- **THEN** unsupported fixture を暗黙 skip しない

### Requirement: score 改善は fallback で隠してはならない

システムは、score 比較を通すために fallback SVG、stub PNG、空出力を使ってはならない（MUST NOT）。

#### Scenario: runtime が失敗する

- **GIVEN** Mermaid または Draw.io runtime が失敗する
- **WHEN** compare を実行する
- **THEN** compare は error first で失敗を報告する
- **THEN** fallback 画像で score を作らない

### Requirement: Jules が小さい cycle で再帰的に改善できる

システムは、score 未達を一括修正ではなく、case 単位の調査、分類、修正、再比較で進められる手順を持たなければならない（MUST）。

#### Scenario: 最初の failing case を切り出す

- **GIVEN** `just drawio-compare-ci 99` または `just drawio-compare-full 99` が失敗する
- **WHEN** Jules が作業を開始する
- **THEN** 最初に失敗した fixture 名、score、比較出力 directory を report に記録する
- **THEN** 該当 fixture を含む最小 fixture directory だけを `just drawio-compare <fixture-dir> 99 tmp/kcf-v0.1.2-score-improvement/<case>` で再実行する
- **THEN** 一度の cycle では一種類の差分だけを修正する

#### Scenario: 原因が分からない

- **GIVEN** 差分原因を renderer、resource、postprocess、reference 特殊ケースへ分類できない
- **WHEN** Jules が次の修正を判断できない
- **THEN** 推測で broad refactor をしない
- **THEN** 見えている差分、触ったファイル、次に疑う場所を report に残して停止する
