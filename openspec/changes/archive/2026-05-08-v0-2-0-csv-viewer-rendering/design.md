## Context

責務再整理により、CSV viewer renderingはKDVへ移譲する。

KDRはCSVをviewer artifactへ変換する責務を持たない。KDRの最終責務はMermaid、Draw.io、PlantUML、mathなどの外部描画と、runtime asset、reference、scoreの維持である。

## Decision

KDR側の `v0-2-0-csv-viewer-rendering` は実装changeとして進めない。

CSV viewerに必要な入力、windowing、diagnostics、表示確認caseは `katana-document-viewer` 側のOpenSpecへ移す。

## Verification

- KDR側にCSV viewer API、fixture、CLI entrypointを追加しない。
- KDV側に移譲先changeが用意されるまで、このchangeは移譲記録として残す。
