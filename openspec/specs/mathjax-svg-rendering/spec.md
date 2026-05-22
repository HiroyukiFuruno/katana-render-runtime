# mathjax-svg-rendering Specification

## Purpose
TBD - created by archiving change v0-3-0-rename-to-katana-render-runtime. Update Purpose after archive.
## Requirements
### Requirement: MathJax runtime は TeX を SVG へ変換しなければならない

システムは、MathJax v4 系 runtime を使って TeX input を SVG output へ変換しなければならない（MUST）。SVG化に失敗した場合は diagnostics 付き raw string を返さなければならない（MUST）。

#### Scenario: display math を SVG 化する

- **WHEN** consumer が `\\int_{0}^{x} \\frac{t^2}{1 + t^4} \\, dt` を display math として渡す
- **THEN** 出力には SVG が含まれる
- **THEN** 出力には width / height / viewBox の描画 metadata が含まれる
- **THEN** raw TeX fallback は含まれない

#### Scenario: inline math を SVG 化する

- **WHEN** consumer が `E = mc^2` を inline math として渡す
- **THEN** 出力には inline 配置に使える SVG が含まれる
- **THEN** raw `mc^2` の text fallback だけを返さない

### Requirement: Markdown parsing は MathJax runtime の責務にしてはならない

システムは、`$...$`、`$$...$$`、fenced `math` の Markdown 構文判定を `katana-render-runtime` の責務にしてはならない（MUST NOT）。Markdown AST の解釈は KDV / KMM 側で行わなければならない（MUST）。

#### Scenario: KDV が数式を渡す

- **GIVEN** KDV が Markdown AST から inline math を検出している
- **WHEN** KDV が runtime に入力する
- **THEN** KDV は外側の `$` marker を剥がした TeX を渡す
- **THEN** runtime は Markdown parser として振る舞わない
- **THEN** runtime は受け取った入力を AST 解析しない

#### Scenario: 単一 wrapper helper を使う

- **WHEN** runtime が互換補助として `$ E = mc^2 $` を正規化する
- **THEN** 外側の `$` だけを剥がして trim する
- **THEN** 文書中の複数 inline math を探索する parser にはしない

### Requirement: MathJax SVG は theme と dark-mode を受け取れなければならない

システムは、MathJax SVG 生成時に complete theme object と dark-mode を受け取れる contract を提供しなければならない（MUST）。

#### Scenario: light theme で数式を描画する

- **WHEN** consumer が light theme と `dark_mode = false` を渡す
- **THEN** MathJax SVG は light theme 上で読める配色になる
- **THEN** 背景は必要に応じて transparent として扱える

#### Scenario: dark theme で数式を描画する

- **WHEN** consumer が dark theme と `dark_mode = true` を渡す
- **THEN** MathJax SVG は dark theme 上で読める配色になる
- **THEN** light theme 固定色へ暗黙 fallback しない

### Requirement: MathJax runtime は失敗時に diagnostics 付き raw string を返さなければならない

システムは、MathJax 実行失敗時に panic せず、diagnostics 付き raw string を返さなければならない（MUST）。Runtime は失敗を隠蔽して成功 SVG として扱ってはならない（MUST NOT）。

#### Scenario: 不正な TeX を渡す

- **WHEN** consumer が MathJax で処理できない TeX を渡す
- **THEN** 出力は raw string を持つ
- **THEN** 出力は失敗 diagnostics を持つ
- **THEN** consumer は raw string を表示して成功扱いするかどうかを自分で判断できる
- **THEN** runtime は silent fallback しない

### Requirement: MathJax bundle は repository と package に含めなければならない

システムは、MathJax runtime bundle を generated artifact として repository 管理し、crates.io package に含めなければならない（MUST）。crate consumer の build 時に Node / Bun / Rollup を要求してはならない（MUST NOT）。

#### Scenario: Package 内容を確認する

- **WHEN** `cargo package -p katana-render-runtime --locked --list` を実行する
- **THEN** MathJax runtime bundle が package に含まれる
- **THEN** checksum manifest に MathJax bundle が含まれる
- **THEN** package consumer は JavaScript toolchain なしで build できる

### Requirement: MathJax JavaScript asset は checksum 管理されなければならない

システムは、MathJax の JavaScript asset を既存 runtime asset と同じ形式で version と checksum により固定しなければならない（MUST）。最新版確認と更新は `just mathjax-latest` / `just mathjax-update <version>` で実行できなければならない（MUST）。

#### Scenario: MathJax asset の checksum を確認する

- **WHEN** `just runtime-asset-check` を実行する
- **THEN** MathJax JavaScript asset の checksum が検証される
- **THEN** Mermaid / Draw.io / PlantUML など既存 runtime asset と同じ検査経路で扱われる

#### Scenario: MathJax asset の最新版と更新を扱う

- **WHEN** `just mathjax-latest` を実行する
- **THEN** 現在固定している version と upstream の latest version を表示する
- **WHEN** `just mathjax-update <version>` を実行する
- **THEN** MathJax JavaScript asset、`.sha256`、Rust 側の固定値が更新される

### Requirement: MathJax SVG出力は DoD に含めなければならない

システムは、MathJax runtime の TeX to SVG 出力を change の完了条件に含めなければならない（MUST）。MathJax bundle を取り込んだだけで完了扱いしてはならない（MUST NOT）。

#### Scenario: MathJax runtime の完了可否を判断する

- **WHEN** `v0-3-0-rename-to-katana-render-runtime` の DoD を確認する
- **THEN** inline math の TeX input が SVG output になる focused test が通っている
- **THEN** display math の TeX input が SVG output になる focused test が通っている
- **THEN** 失敗時に diagnostics 付き raw string を返す focused test が通っている
- **THEN** runtime は入力の AST 解析を行っていない
