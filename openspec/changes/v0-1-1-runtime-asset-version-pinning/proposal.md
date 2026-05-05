## Why

v0.1.0 は KatanA 既存 rendering/export runtime の純粋な移植に限定する。移植後に残る Mermaid.js / Draw.io.js の runtime asset 管理課題は、v0.1.1 の小規模 patch として解決する。

Mermaid.js と Draw.io.js の取り込み version が曖昧なままだと、reference score、cache fingerprint、CI 再現性、KatanA consumer integration が揺れる。kcf が runtime asset の version 固定、最新版確認、取り込み、checksum、reference snapshot 更新を所有する。

## What Changes

- Mermaid.js の取り込み version を kcf 側で固定する
- Draw.io.js の取り込み version を kcf 側で固定する
- 現在取り込める Mermaid.js / Draw.io.js の最新版を確認する just recipe を追加する
- 指定 version を取り込み、checksum、manifest、reference snapshot を更新する just recipe を追加する
- runtime metadata と cache fingerprint が固定 version と checksum を参照するようにする
- v0.1.0 transfer の実装を壊さず、runtime asset 管理だけを patch として追加する

## Non-Goals

- Mermaid / Draw.io renderer 自体を作り直さない
- HTML / PDF / PNG / JPEG export の移植をここで行わない
- ImageMagick score の改善は v0.4.x に送る
- 公開 CLI surface の整理は v0.5.0 に送る

## Capabilities

### New Capabilities

- `runtime-asset-versioning`: Mermaid.js / Draw.io.js の version 固定、最新版確認、取り込み更新、checksum 管理

## Impact

- `vendor/mermaid/<version>/` — Mermaid.js bundle と checksum
- `vendor/drawio/<version>/` — Draw.io.js bundle、resource manifest、checksum
- `Justfile` — latest check と update recipe
- `crates/katana-canvas-forge/src/mermaid/` — runtime metadata / checksum integration
- `crates/katana-canvas-forge/src/drawio/` — runtime metadata / checksum integration
- `tests/reference/` — version 更新後の reference snapshot 検証
