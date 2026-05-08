## Context

責務再整理により、Office viewer renderingはKDVへ移譲する。

KCFはWord / Excel / PPTXをviewer表示用artifactへ変換する責務を持たない。必要な場合はKDVがviewer入力、artifact、diagnostics、表示確認caseを定義する。

## Decision

KCF側の `v0-4-0-office-viewer-rendering` は実装changeとして進めない。

Office viewerに必要な入力、page / sheet / slide metadata、diagnostics、reference、表示確認caseは `katana-document-viewer` 側のOpenSpecへ移す。

## Verification

- KCF側にOffice viewer API、fixture、CLI entrypointを追加しない。
- KDV側に移譲先changeが用意されるまで、このchangeは移譲記録として残す。
