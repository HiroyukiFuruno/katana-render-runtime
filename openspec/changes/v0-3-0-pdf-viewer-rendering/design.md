## Context

責務再整理により、PDF viewer renderingはKDVへ移譲する。

KCFは既存PDF exportをKDV同等機能が入るまで維持するが、既存PDFファイルをviewer表示用artifactへ変換する責務は持たない。

## Decision

KCF側の `v0-3-0-pdf-viewer-rendering` は実装changeとして進めない。

PDF viewerに必要なpage artifact、metadata、diagnostics、windowing、表示確認caseは `katana-document-viewer` 側のOpenSpecへ移す。

## Verification

- KCF側にPDF viewer API、fixture、CLI entrypointを追加しない。
- KDV側に移譲先changeが用意されるまで、このchangeは移譲記録として残す。
