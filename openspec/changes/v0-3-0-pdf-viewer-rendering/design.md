## Context

v0.3.0 は PDF viewer rendering の change である。

PDF export と PDF viewer rendering は逆方向の処理である。export は kcf が PDF を生成する。viewer rendering は既存 PDF を読み、画面表示できる page artifact に変換する。

## Goals

- PDF を page 単位で render できる
- page range、scale、background を指定できる
- page metadata と diagnostics を返せる
- KatanA UI に依存しない API にする
- viewer e2e で実表示を確認できる

## Non-Goals

- PDF 編集、annotation、signature、form 入力
- text extraction、search、OCR
- PDF export の内部刷新
- public CLI contract の固定

## Rendering Backend

実装時には PDF rendering backend を 1 つ選び、repository 内で初期化方法を固定する。

候補は PDFium、Poppler、MuPDF などだが、v0.3.0 の実装前に次を満たすものを選ぶ。

- macOS / Linux / CI で再現できる
- license が kcf の配布方針と衝突しない
- page image を deterministic に生成できる
- backend binary / dynamic library の取得方法を document 化できる

backend が利用できない場合は、暗黙 fallback で空 image を返さず、構造化 error を返す。

## Public Boundary

PDF viewer rendering は viewer DTO を使う。

- `ViewerRenderInput`: PDF path または bytes
- `PdfRenderOptions`: page range、scale、background、max pages
- `PdfPageArtifact`: page index、image artifact path または bytes、width、height、scale
- `PdfDocumentMetadata`: page count、title、page sizes、encrypted flag
- `ViewerDiagnostics`: backend missing、password required、invalid PDF、unsupported feature

## Page Windowing

既定では先頭 page または小さな page range だけを render する。

全 page render は明示 option にする。巨大 PDF で CI が不安定にならないよう、fixture と timeout policy を tasks に含める。

## Viewer E2E

viewer e2e は PDF を直接表示してもよいが、初期完了条件は page artifact を表示できることとする。

PDF 直接表示は UI 側の責務になりやすいため、kcf の正本は page artifact 生成と metadata / diagnostics に置く。

## Verification

- valid PDF fixture の page artifact 比較
- rotated / multi-page / large fixture の metadata 検証
- invalid / encrypted PDF の error 検証
- viewer e2e screenshot
- `just check`
- `npx -y @fission-ai/openspec validate v0-3-0-pdf-viewer-rendering --strict`
