## Context

v0.1.0 は純粋な transfer。KatanA 既存実装を kcf に移すことだけを完了条件にする。

v0.1.1 は、その transfer 後に残る runtime asset 管理の課題を解決する patch。Mermaid.js / Draw.io.js の取り込み version、checksum、更新 recipe、reference snapshot 更新を kcf が所有する。

## Goals

- Mermaid.js の取り込み version を明示する
- Draw.io.js の取り込み version を明示する
- 最新版確認の just recipe を提供する
- 指定 version 取り込みの just recipe を提供する
- checksum と runtime metadata を実出力の検証に使う
- reference snapshot 更新を version 更新と同じ操作で実行できるようにする

## Non-Goals

- renderer の忠実移植は v0.1.0 で扱う
- score 改善は v0.4.x で扱う
- 公開 CLI の UX 固定は v0.5.0 で扱う

## Asset Ownership

kcf は runtime asset を repository 内で所有する。

- Mermaid.js: `vendor/mermaid/<version>/`
- Draw.io.js: `vendor/drawio/<version>/`
- checksum: asset と同じ directory に `.sha256` として配置する
- manifest: Draw.io resource や runtime asset の一覧を kcf 内で管理する

## Just Recipe Design

recipe は少なくとも次を提供する。

- Mermaid.js の latest version を確認する
- Draw.io.js の latest version を確認する
- Mermaid.js の指定 version を取り込む
- Draw.io.js の指定 version を取り込む
- 取り込み後に checksum を更新する
- 取り込み後に reference snapshot を再生成する
- 取り込み後に compare を実行し、score 低下を検知する

## Separation From v0.1.0

v0.1.0 は KatanA 既存実装を移すだけ。runtime asset 管理が未整理でも、既存実装相当で動く状態までを v0.1.0 とする。

v0.1.1 は、v0.1.0 で移した runtime asset を kcf の所有物として固定し、更新可能にする。
