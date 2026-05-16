## ADDED Requirements

### Requirement: Generated runtime bundle checksum は minify / mangle 後の最終 artifact を固定しなければならない

システムは、KDR 生成 runtime bundle の checksum を minify / mangle 後の最終 `*.min.js` artifact に対して固定しなければならない（MUST）。Checksum は source bundle、debug bundle、vendor asset checksum と混同してはならない（MUST NOT）。

#### Scenario: Runtime bundle checksum を検証する

- **WHEN** `just runtime-asset-check` または同等の checksum 検証を実行する
- **THEN** `mermaid-runtime.min.js`、`drawio-runtime.min.js`、`zenuml-runtime.min.js` の checksum は minify / mangle 後の成果物と一致する
- **THEN** `runtime-bundles.sha256` は最終 generated bundle の checksum を記録する
- **THEN** upstream vendor asset の checksum と KDR generated bundle の checksum は別々に report される

#### Scenario: Bundle を再生成する

- **WHEN** runtime bundle 生成 recipe を実行する
- **THEN** ESM source、`package.json` `imports`、Rollup config、Terser config から deterministic に最終 `*.min.js` が再生成される
- **THEN** Terser config は entry I/F の reserved name、comment 除去、source map 無効化、LF 改行を固定している
- **THEN** 再生成した checksum が repository 管理済み checksum と一致しない場合は検証が失敗する
