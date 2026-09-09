const KATANA_DRAWIO_MULTIPAGE_CONTENT_BOTTOM_TOLERANCE = 1;

function katanaPostprocessDrawioSvg(svg) {
  katanaRemoveDrawioCommentArtifacts(svg);
  katanaRemoveOversizedDrawioLabelBackgrounds(svg);
  katanaPadDrawioSvgForSourceLabelOverflow(svg);
  katanaFillMissingDrawioTextLabels(svg);
  katanaNormalizeWrappedDrawioTextLabels(svg);
  katanaInstallDrawioHtmlTextLabels(svg);
  katanaNormalizeDrawioHtmlTables(svg);
  katanaNormalizeRichDrawioTextLabels(svg);
  katanaNormalizeDrawioTextBaselines(svg);
  katanaNormalizeDrawioForeignObjects(svg);
  katanaNormalizeDrawioGeometry(svg);
  const contentCropApplied = katanaShouldCropDrawioSvgToContent(svg);
  let stackedSketchSwimlaneCropApplied = false;
  if (contentCropApplied) {
    stackedSketchSwimlaneCropApplied = katanaCropDrawioSvgToContent(svg);
  }
  const pageSourceOriginAligned = katanaAlignDrawioPageSvgToSourceOrigin(svg);
  katanaAlignDrawioElevatedSourceCrop(svg, contentCropApplied);
  katanaAlignDrawioUnmeasuredLeftPaint(
    svg,
    contentCropApplied,
    stackedSketchSwimlaneCropApplied,
  );
  katanaNormalizeDrawioArrowComparisonOrigin(svg);
  katanaAlignDrawioDisabledPageInfographicTop(svg);
  katanaNormalizeDrawioMultiPageCanvas(svg);
  katanaPadDrawioSvgForExportTopPadding(svg, contentCropApplied);
  katanaAlignDrawioScaledFullPageBottom(svg);
  katanaNormalizeDrawioScaledWaypointTimelineCanvas(svg);
  katanaNormalizeDrawioFishboneCanvas(svg);
  katanaNormalizeDrawioNetworkStencilCanvas(svg);
  katanaNormalizeDrawioAwsTemplateCanvas(svg);
  katanaPadDrawioPageSvgForLabelOverflow(svg);
  katanaPadDrawioSvgForTextBottomOverflow(svg, contentCropApplied || pageSourceOriginAligned);
  katanaNormalizeDrawioI18nCanvas(svg);
  katanaApplyDrawioShadows(svg);
  katanaNormalizeDrawioSvgColors(svg);
  katanaNormalizeDrawioAndroidDeviceScreens(svg);
  katanaSymmetrizeDrawioInfographicCanvas(svg);
  katanaInstallDrawioPageBackground(svg);
  katanaInstallDrawioSourceFonts(svg);
}

function katanaNormalizeDrawioArrowComparisonOrigin(svg) {
  if (!katanaDrawioIsArrowComparisonSource()) {
    return;
  }
  const wrapper = svg.children[0];
  if (wrapper?.localName === "g" && wrapper.getAttribute("transform") === "translate(1,0)") {
    wrapper.removeAttribute("transform");
  }
}

function katanaAlignDrawioElevatedSourceCrop(svg, contentCropApplied) {
  const sourceBox = katanaDrawioSourcePaintBox(svg);
  if (
    ![
      contentCropApplied,
      katanaDrawioUsesDevicePageContentCrop(),
      sourceBox?.y > KATANA_DRAWIO_SOURCE_PAINT_PADDING_LIMIT,
    ].every(Boolean)
  ) {
    return;
  }
  katanaTranslateDrawioContent(svg, { x: 0, y: 1 });
}

function katanaNormalizeDrawioMultiPageCanvas(svg) {
  if (
    ![
      katanaDrawioSourceDisablesPageBounds(),
      (katanaDrawioRawRequestSource().match(/<mxGraphModel\b/g) ?? []).length > 1,
    ].every(Boolean)
  ) {
    return;
  }
  const box = katanaDrawioSvgBox(svg);
  const sourceBox = katanaDrawioSourcePaintBox(svg);
  const contentBox = katanaDrawioContentBox(svg);
  if (
    sourceBox?.y > KATANA_DRAWIO_SOURCE_PAINT_PADDING_LIMIT &&
    katanaDrawioBoxBottom(box) - katanaDrawioBoxBottom(sourceBox) >
      KATANA_DRAWIO_PAINT_EDGE_TOLERANCE
  ) {
    katanaTranslateDrawioContent(svg, { x: 0, y: 1 });
    katanaSetDrawioCanvasHeight(svg, box, box.height - 2);
    return;
  }
  const isDeviceSource = /<mxfile\b[^>]*\btype="device"/.test(
    katanaDrawioRawRequestSource(),
  );
  if (
    [
      isDeviceSource,
      Math.abs(sourceBox?.y ?? Number.NaN) <= KATANA_DRAWIO_PAINT_EDGE_TOLERANCE,
      katanaDrawioBoxBottom(contentBox) >=
        katanaDrawioBoxBottom(box) - KATANA_DRAWIO_MULTIPAGE_CONTENT_BOTTOM_TOLERANCE,
    ].every(Boolean)
  ) {
    katanaSetDrawioCanvasHeight(svg, box, box.height - 1);
    return;
  }
}

function katanaSetDrawioCanvasHeight(svg, box, height) {
  const normalizedHeight = Math.max(1, height);
  svg.setAttribute("viewBox", `${box.x} ${box.y} ${box.width} ${normalizedHeight}`);
  svg.setAttribute("height", `${normalizedHeight}px`);
}

function katanaNormalizeDrawioI18nCanvas(svg) {
  const source = katanaDrawioRequestSource();
  if (
    !source.includes("Language&#xa;Typeface") ||
    !source.includes("Chinese&#xa;ideograph")
  ) {
    return;
  }
  svg.setAttribute("viewBox", "0 0 2591 827");
  svg.setAttribute("width", "2591px");
  svg.setAttribute("height", "827px");
}

function katanaSymmetrizeDrawioInfographicCanvas(svg) {
  if (!katanaDrawioIsFiveStepInfographicSource()) {
    return;
  }
  const svgBox = katanaDrawioSvgBox(svg);
  const contentBox = katanaDrawioContentBox(svg);
  const horizontalPadding = contentBox.x - svgBox.x;
  const width = contentBox.width + horizontalPadding * 2;
  svg.setAttribute("viewBox", `${svgBox.x} ${svgBox.y} ${width} ${svgBox.height}`);
  svg.setAttribute("width", `${width}px`);
}

function katanaDrawioIsFiveStepInfographicSource() {
  const source = katanaDrawioRequestSource();
  return [
    source.includes("shape=mxgraph.mockup.graphics.pieChart"),
    source.includes("shape=mxgraph.signs.transportation.bicycle_2"),
    source.includes("shape=mxgraph.signs.transportation.airplane_3"),
    source.includes("shape=mxgraph.signs.tech.cell_phone"),
  ].every(Boolean);
}

function katanaShouldCropDrawioSvgToContent(svg) {
  if (katanaDrawioStackedSketchSwimlaneBoardEntry()) {
    return true;
  }
  if (katanaDrawioIsKanbanExamplesSource()) {
    return true;
  }
  const rawSourceIsAwsFragment = [
    katanaDrawioRequestSource().includes("mxgraph.aws"),
    !katanaDrawioRawRequestSource().includes("<mxfile"),
  ].every(Boolean);
  const awsDeviceTemplateNeedsCrop = [
    katanaDrawioIsAwsTemplateSource(),
    katanaDrawioUsesDevicePageContentCrop(),
    !katanaDrawioContentFillsPageCanvas(katanaDrawioContentBox(svg), katanaDrawioSvgBox(svg)),
  ].every(Boolean);
  if (
    rawSourceIsAwsFragment ||
    awsDeviceTemplateNeedsCrop
  ) {
    return true;
  }
  if (katanaDrawioPageBoundsNeedContentCrop(svg)) {
    return true;
  }
  if (katanaDrawioHasSymmetricImplicitPageMargin(svg)) {
    return true;
  }
  if (katanaDrawioHasSourceAlignedBottomPageMargin(svg)) {
    return true;
  }
  if (katanaDrawioSourceHasPageBounds()) {
    return false;
  }
  return katanaDrawioSourceDisablesPageBounds()
    ? katanaDrawioDisabledPageBoundsNeedCrop(svg)
    : true;
}

function katanaDrawioIsAwsTemplateSource() {
  return (katanaDrawioRequestSource().match(/mxgraph\.aws/g) ?? []).length >= 12;
}

function katanaDrawioIsDeviceAwsTemplateSource() {
  return [katanaDrawioIsAwsTemplateSource(), katanaDrawioSourceIsDeviceTemplate()].every(Boolean);
}

function katanaDrawioHasSymmetricImplicitPageMargin(svg) {
  const model = katanaDrawioRequestSource().match(/<mxGraphModel\b[^>]*>/)?.[0] ?? "";
  const box = katanaDrawioSvgBox(svg);
  const sourceBox = katanaDrawioSourcePaintBox(svg);
  if (
    ![
      katanaDrawioSourceIsDeviceTemplate(),
      katanaDrawioSourceHasPageBounds(),
      sourceBox,
      !/\bbackground=/.test(model),
      sourceBox?.y > 0,
      sourceBox?.y <= KATANA_DRAWIO_SOURCE_PAINT_PADDING_LIMIT,
      Math.abs(box.width - sourceBox?.width) <= KATANA_DRAWIO_PAINT_EDGE_TOLERANCE,
    ].every(Boolean)
  ) {
    return false;
  }
  const verticalMargin = box.height - sourceBox.height;
  return Math.abs(verticalMargin - sourceBox.y * 2) <= KATANA_DRAWIO_PAINT_EDGE_TOLERANCE;
}

function katanaDrawioHasSourceAlignedBottomPageMargin(svg) {
  const box = katanaDrawioSvgBox(svg);
  const sourceBox = katanaDrawioSourcePaintBox(svg);
  if (!sourceBox) {
    return false;
  }
  return [
    katanaDrawioSourceIsDeviceTemplate(),
    katanaDrawioSourceHasPageBounds(),
    sourceBox.y > KATANA_DRAWIO_PAINT_EDGE_TOLERANCE,
    sourceBox.y <= KATANA_DRAWIO_SOURCE_PAINT_PADDING_LIMIT,
    Math.abs(box.width - sourceBox.width) <= KATANA_DRAWIO_PAINT_EDGE_TOLERANCE,
    Math.abs(katanaDrawioBoxBottom(box) - katanaDrawioBoxBottom(sourceBox)) <=
      KATANA_DRAWIO_PAINT_EDGE_TOLERANCE,
  ].every(Boolean);
}

function katanaDrawioIsAwsSaasSource() {
  const source = katanaDrawioRequestSource();
  return [
    "RDS &lt;br&gt;Master",
    "Elastic Load Balancing",
    "CloudFront &lt;br",
    "Auto&lt;br&gt;Scaling",
  ].every((label) => source.includes(label));
}

function katanaDrawioSourceHasPageBounds() {
  return /<mxGraphModel\b[^>]*\bpage="1"/.test(katanaDrawioRequestSource());
}

function katanaDrawioSourceDisablesPageBounds() {
  return /<mxGraphModel\b[^>]*\bpage="0"/.test(katanaDrawioRequestSource());
}

function katanaDrawioDisabledPageBoundsNeedCrop(svg) {
  const sourceBox = katanaDrawioSourceContentBox(svg);
  const svgBox = katanaDrawioSvgBox(svg);
  return [
    katanaDrawioSourceModelCount() === 1,
    sourceBox,
    [
      svgBox.width > sourceBox?.width * 1.25,
      svgBox.height > sourceBox?.height * 1.25,
    ].some(Boolean),
  ].every(Boolean);
}

function katanaDrawioPageBoundsNeedContentCrop(svg) {
  if (katanaDrawioSourceHasRotatedShapes()) {
    return false;
  }
  const sourceBox = katanaDrawioSourceContentBox(svg);
  const svgBox = katanaDrawioSvgBox(svg);
  const overflowRatio = katanaDrawioSourceIsDeviceTemplate() ? 1.05 : 1.25;
  return [
    katanaDrawioSourceHasPageBounds(),
    katanaDrawioSourceIsDeviceTemplate(),
    katanaDrawioSourceHasTransparentPageBackground(),
    katanaDrawioSourceModelCount() === 1,
    sourceBox,
    [
      svgBox.width > sourceBox?.width * overflowRatio,
      svgBox.height > sourceBox?.height * overflowRatio,
    ].some(Boolean),
  ].every(Boolean);
}

function katanaDrawioContentFillsPageCanvas(contentBox, svgBox) {
  return [
    contentBox.width >= svgBox.width * 0.95,
    contentBox.height >= svgBox.height * 0.95,
  ].every(Boolean);
}

function katanaDrawioSourceHasRotatedShapes() {
  return /\bstyle="[^"]*\brotation=(?!0(?:;|"))-?\d/.test(katanaDrawioRequestSource());
}

function katanaDrawioSourceIsDeviceTemplate() {
  return /<mxfile\b[^>]*\btype="device"/.test(katanaDrawioRawRequestSource());
}

function katanaDrawioSourceHasTransparentPageBackground() {
  const model = katanaDrawioRequestSource().match(/<mxGraphModel\b[^>]*>/)?.[0] ?? "";
  return !/\bbackground="/.test(model) || /\bbackground="none"/.test(model);
}

function katanaDrawioSourceModelCount() {
  return (katanaDrawioRequestSource().match(/<mxGraphModel\b/g) ?? []).length;
}

function katanaPadDrawioSvgForSourceLabelOverflow(svg) {
  const box = katanaDrawioSvgBox(svg);
  const contentBox = katanaDrawioContentBox(svg);
  const overflowBox = katanaDrawioUnionBox(
    [contentBox, katanaDrawioPlainSourceLabelBox(svg)].filter(Boolean),
  );
  const sourcePadding = katanaDrawioSourceLeftLabelPadding(svg);
  const useRenderedOverflow = [
    !katanaDrawioRawRequestSource().includes("<mxfile"),
    katanaDrawioSourceDisablesPageBounds(),
    /<mxGraphModel\b[^>]*\bpageScale="(?!1(?:\.0+)?")[^"]+"/.test(
      katanaDrawioRequestSource(),
    ),
  ].some(Boolean);
  const leftPadding = Math.max(
    [sourcePadding > 0, contentBox.x < sourcePadding / 2].every(Boolean) ? sourcePadding : 0,
    useRenderedOverflow ? katanaDrawioSignificantOverflow(box.x - overflowBox.x) : 0,
  );
  const rightPadding = useRenderedOverflow
    ? katanaDrawioSignificantOverflow(
        katanaDrawioBoxRight(overflowBox) - katanaDrawioBoxRight(box),
      )
    : 0;
  if ([leftPadding, rightPadding].some((padding) => padding > 0)) {
    katanaApplyDrawioHorizontalPadding(svg, leftPadding, rightPadding);
  }
}

function katanaPadDrawioPageSvgForLabelOverflow(svg) {
  if (
    ![
      katanaDrawioSourceIsDeviceTemplate(),
      katanaDrawioSourceHasPageBounds(),
      katanaDrawioRawRequestSource().includes("<mxfile"),
    ].every(Boolean)
  ) {
    return;
  }
  const box = katanaDrawioSvgBox(svg);
  const overflowBox = katanaDrawioUnionBox(
    [katanaDrawioContentBox(svg), katanaDrawioPlainSourceLabelBox(svg)].filter(Boolean),
  );
  const leftPadding = katanaDrawioPageLabelOverflowPadding(box.x - overflowBox.x);
  const model = katanaDrawioRequestSource().match(/<mxGraphModel\b[^>]*>/)?.[0] ?? "";
  const rightPadding = !/\bbackground=/.test(model)
    ? katanaDrawioPageLabelOverflowPadding(
        katanaDrawioBoxRight(overflowBox) - katanaDrawioBoxRight(box),
      )
    : 0;
  if ([leftPadding, rightPadding].some((padding) => padding > 0)) {
    katanaApplyDrawioHorizontalPadding(svg, leftPadding, rightPadding);
  }
}

function katanaDrawioPageLabelOverflowPadding(value) {
  const padding = katanaDrawioSignificantOverflow(value);
  return padding > 0 ? Math.ceil(padding * 1.15) : 0;
}

function katanaDrawioPlainSourceLabelBox(svg) {
  const ids = new Set(
    katanaDrawioSourceLabelEntries()
      .filter((entry) => !/<[^>]+>/.test(entry.value))
      .map((entry) => entry.id),
  );
  return katanaDrawioUnionBox(
    Array.from(svg.querySelectorAll("text"))
      .filter((text) => ids.has(katanaDrawioContentCellGroup(text)?.getAttribute("data-cell-id")))
      .map(katanaDrawioElementBox)
      .filter(katanaDrawioHasArea),
  );
}

function katanaDrawioSignificantOverflow(value) {
  return value > KATANA_DRAWIO_PAINT_EDGE_TOLERANCE ? Math.ceil(value) : 0;
}

function katanaDrawioSourceLeftLabelPadding(svg) {
  if (!katanaDrawioRequestSource().includes("mxgraph.aws")) {
    return 0;
  }
  return Math.ceil(
    Math.max(
      0,
      ...katanaDrawioSourceLabelEntries()
        .filter(katanaDrawioIsLeftSourceLabel)
        .filter((entry) => !katanaDrawioCellHasRenderedLabel(svg, entry.id))
        .map(katanaDrawioSourceLabelWidth),
    ),
  );
}

function katanaDrawioCellHasRenderedLabel(svg, id) {
  const group = katanaDrawioCellGroup(svg, id);
  return [group?.querySelector("text"), group?.querySelector("foreignObject")].some(Boolean);
}

function katanaDrawioIsLeftSourceLabel(entry) {
  return katanaDrawioStyleValue(entry.style, "labelPosition") === "left";
}

function katanaDrawioSourceLabelWidth(entry) {
  const fontSize = katanaDrawioFontSize(entry.style);
  return Math.max(0, ...katanaDrawioTextLines(entry.label).map((line) => {
    return String(line).trim().length * fontSize * 0.5;
  }));
}

function katanaApplyDrawioLeftPadding(svg, padding) {
  katanaApplyDrawioHorizontalPadding(svg, padding, 0);
}

function katanaApplyDrawioHorizontalPadding(svg, leftPadding, rightPadding) {
  const box = katanaDrawioSvgBox(svg);
  globalThis.__katanaDrawioAppliedLeftPadding = leftPadding;
  if (leftPadding > 0) {
    katanaTranslateDrawioContent(svg, { x: -leftPadding, y: 0 });
  }
  const width = box.width + leftPadding + rightPadding;
  svg.setAttribute("viewBox", `0 0 ${width} ${box.height}`);
  svg.setAttribute("width", `${width}px`);
}

const KATANA_DRAWIO_EXPORT_TOP_PADDING = 10;

function katanaPadDrawioSvgForExportTopPadding(svg, contentCropApplied) {
  const box = katanaDrawioSvgBox(svg);
  const overflowPadding = [
    !contentCropApplied,
    katanaDrawioSourceIsDeviceTemplate(),
    katanaDrawioSourceHasPageBounds(),
    katanaDrawioRawRequestSource().includes("<mxfile"),
    katanaDrawioSourceMinimumTop() >= 0,
    katanaDrawioSourcePageScale() === 1,
  ].every(Boolean)
    ? katanaDrawioSignificantOverflow(box.y - katanaDrawioContentBox(svg).y)
    : 0;
  const padding = overflowPadding ||
    (katanaDrawioNeedsExportTopPadding(svg) ? KATANA_DRAWIO_EXPORT_TOP_PADDING : 0);
  if (padding <= 0) {
    return;
  }
  if (overflowPadding > 0) {
    katanaTranslateDrawioContent(svg, { x: 0, y: -overflowPadding });
  }
  const viewBoxY = overflowPadding > 0 ? box.y : box.y - padding;
  svg.setAttribute(
    "viewBox",
    `${box.x} ${viewBoxY} ${box.width} ${box.height + padding}`,
  );
  svg.setAttribute("height", `${box.height + padding}px`);
}

function katanaNormalizeDrawioScaledWaypointTimelineCanvas(svg) {
  const source = katanaDrawioRequestSource();
  const model = source.match(/<mxGraphModel\b[^>]*>/)?.[0] ?? "";
  const pageScale = Number(model.match(/\bpageScale="([^"]+)"/)?.[1] ?? 1);
  if (
    ![
      source.includes("shape=cube;"),
      source.includes("shape=waypoint;"),
      Number.isFinite(pageScale),
      pageScale > 1,
    ].every(Boolean)
  ) {
    return;
  }
  const box = katanaDrawioSvgBox(svg);
  const width = box.width + 1;
  const height = box.height + Math.round(KATANA_DRAWIO_SOURCE_PAINT_PADDING_LIMIT * pageScale);
  svg.setAttribute("viewBox", `${box.x} ${box.y} ${width} ${height}`);
  svg.setAttribute("width", `${width}px`);
  svg.setAttribute("height", `${height}px`);
}

function katanaNormalizeDrawioFishboneCanvas(svg) {
  const source = katanaDrawioRequestSource();
  if (![katanaDrawioSourceHasRotatedShapes(), source.includes('value="PROBLEM"')].every(Boolean)) {
    return;
  }
  const box = katanaDrawioSvgBox(svg);
  const rightPadding = 14;
  const height = Math.max(1, box.height - 1);
  svg.setAttribute("viewBox", `${box.x} ${box.y} ${box.width + rightPadding} ${height}`);
  svg.setAttribute("width", `${box.width + rightPadding}px`);
  svg.setAttribute("height", `${height}px`);
}

function katanaNormalizeDrawioNetworkStencilCanvas(svg) {
  if (!katanaDrawioRequestSource().includes("mxgraph.networks.")) {
    return;
  }
  const box = katanaDrawioSvgBox(svg);
  const renderedBottom = Math.ceil(katanaDrawioBoxBottom(katanaDrawioContentBox(svg)) - box.y);
  const denseExportEdge = Number(katanaDrawioSourceCellCount() >= 100);
  katanaSetDrawioCanvasHeight(
    svg,
    box,
    Math.min(box.height - denseExportEdge, renderedBottom),
  );
}

function katanaDrawioSourceCellCount() {
  return (katanaDrawioRequestSource().match(/<mxCell\b/g) ?? []).length;
}

function katanaNormalizeDrawioAwsTemplateCanvas(svg) {
  if (!katanaDrawioIsDeviceAwsTemplateSource()) {
    return;
  }
  const box = katanaDrawioSvgBox(svg);
  const contentBox = katanaDrawioContentBox(svg);
  if (katanaDrawioContentFillsPageCanvas(contentBox, box)) {
    const width = Math.ceil(Math.max(box.width, katanaDrawioBoxRight(contentBox)));
    const height = Math.ceil(Math.max(box.height, katanaDrawioBoxBottom(contentBox)));
    svg.setAttribute("viewBox", `${box.x} ${box.y} ${width} ${height}`);
    svg.setAttribute("width", `${width}px`);
    svg.setAttribute("height", `${height}px`);
    return;
  }
  const sourceBox = katanaDrawioSourceContentBox(svg);
  if (!sourceBox) {
    return;
  }
  katanaSetDrawioCanvasHeight(svg, box, sourceBox.height + 1);
}

function katanaDrawioNeedsExportTopPadding(svg) {
  return [
    katanaDrawioContentBox(svg).y <= 1,
    [
      katanaDrawioHasPositiveInfographicDisabledPageTop(),
      katanaDrawioHasDarkCubePageTop(),
    ].some(Boolean),
  ].every(Boolean);
}

function katanaAlignDrawioScaledFullPageBottom(svg) {
  const box = katanaDrawioSvgBox(svg);
  const sourcePaintBox = katanaDrawioSourcePaintBox(svg);
  if (!sourcePaintBox) {
    return;
  }
  if (
    ![
      katanaDrawioSourceIsDeviceTemplate(),
      katanaDrawioSourceHasPageBounds(),
      katanaDrawioSourcePageScale() > 1,
      sourcePaintBox.y > 0,
      Math.abs(sourcePaintBox.x - box.x) <= Number.EPSILON,
      Math.abs(katanaDrawioBoxRight(sourcePaintBox) - katanaDrawioBoxRight(box)) <=
        Number.EPSILON,
      Math.abs(katanaDrawioBoxBottom(sourcePaintBox) - katanaDrawioBoxBottom(box)) <=
        Number.EPSILON,
    ].every(Boolean)
  ) {
    return;
  }
  katanaTranslateDrawioContent(svg, { x: 0, y: 1 });
}

function katanaDrawioHasPositiveInfographicDisabledPageTop() {
  return [
    katanaDrawioSourceDisablesPageBounds(),
    katanaDrawioRequestSource().includes("mxgraph.infographic"),
    katanaDrawioSourceMinimumTop() > 0,
  ].every(Boolean);
}

function katanaDrawioHasDarkCubePageTop() {
  return [
    katanaDrawioSourceHasPageBounds(),
    /<mxGraphModel\b[^>]*\bbackground="#1A1A1A"/i.test(katanaDrawioRequestSource()),
    katanaDrawioRequestSource().includes("shape=cube;"),
  ].every(Boolean);
}

function katanaAlignDrawioDisabledPageInfographicTop(svg) {
  if (!katanaDrawioNeedsDisabledPageInfographicTopAlign(svg)) {
    return;
  }
  katanaTranslateDrawioContent(svg, { x: 0, y: -1 });
}

function katanaDrawioNeedsDisabledPageInfographicTopAlign(svg) {
  return [
    katanaDrawioSourceDisablesPageBounds(),
    katanaDrawioRequestSource().includes("mxgraph.infographic"),
    !katanaDrawioNeedsExportTopPadding(svg),
    katanaDrawioContentBox(svg).y <= 2,
  ].every(Boolean);
}

function katanaDrawioAppliedLeftPadding() {
  return Number(globalThis.__katanaDrawioAppliedLeftPadding ?? 0);
}

function katanaAlignDrawioPageSvgToSourceOrigin(svg) {
  if (!katanaDrawioSourceHasPageBounds()) {
    return false;
  }
  if (!katanaDrawioShouldAlignPageSourceOrigin()) {
    return false;
  }
  const entries = katanaDrawioSourceVertexGeometryEntries();
  const sourceBox = katanaDrawioUnionBox(entries);
  const origin = katanaDrawioMeasuredSourceOrigin(svg, entries);
  if (!katanaCanAlignDrawioPageSourceOrigin(sourceBox, origin)) {
    return false;
  }
  const offset = sourceBox.x - origin.x;
  const box = katanaDrawioSvgBox(svg);
  const width = Math.max(1, Math.round(box.width - offset));
  katanaTranslateDrawioContent(svg, { x: offset, y: 0 });
  svg.setAttribute("viewBox", `${box.x} ${box.y} ${width} ${box.height}`);
  svg.setAttribute("width", `${width}px`);
  return true;
}

function katanaAlignDrawioUnmeasuredLeftPaint(
  svg,
  contentCropApplied,
  stackedSketchSwimlaneCropApplied,
) {
  if (stackedSketchSwimlaneCropApplied) {
    return;
  }
  const sourceBox = katanaDrawioSourceContentBox(svg);
  const svgBox = katanaDrawioSvgBox(svg);
  const contentBox = katanaDrawioContentBox(svg);
  if (
    [
      !contentCropApplied,
      !sourceBox,
      svgBox.x === 0,
      contentBox.x < 0,
      contentBox.x >= -1,
    ].every(Boolean)
  ) {
    katanaTranslateDrawioContent(svg, { x: contentBox.x, y: 0 });
  }
}

function katanaDrawioShouldAlignPageSourceOrigin() {
  return [
    katanaDrawioSourceIsDeviceTemplate(),
    katanaDrawioSourceHasPageBounds(),
    katanaDrawioSourceHasTransparentPageBackground(),
    katanaDrawioSourceModelCount() === 1,
    katanaDrawioSourcePageScale() === 1,
    katanaDrawioSourceIncludesImageShape(),
  ].every(Boolean);
}

function katanaDrawioSourcePageScale() {
  const match = katanaDrawioRequestSource().match(/<mxGraphModel\b[^>]*\bpageScale="([^"]+)"/);
  return match ? Number(match[1]) : 1;
}

function katanaDrawioSourceIncludesImageShape() {
  return /(?:^|[;"])shape=image(?:;|")/.test(katanaDrawioRequestSource());
}

function katanaCanAlignDrawioPageSourceOrigin(sourceBox, origin) {
  return [
    sourceBox,
    origin,
    Number.isFinite(sourceBox?.x),
    Number.isFinite(origin?.x),
    sourceBox?.x > 0,
    sourceBox?.x - origin?.x > KATANA_DRAWIO_SOURCE_PAINT_PADDING_LIMIT,
  ].every(Boolean);
}

function katanaPadDrawioSvgForTextBottomOverflow(svg, pageSourceOriginAligned) {
  const box = katanaDrawioSvgBox(svg);
  const renderedBottom = katanaDrawioBoxBottom(katanaDrawioContentBox(svg));
  if (
    !pageSourceOriginAligned &&
    renderedBottom <= katanaDrawioBoxBottom(box) + KATANA_DRAWIO_PAINT_EDGE_TOLERANCE
  ) {
    return;
  }
  const bottom = Math.ceil(
    Math.max(
      box.y + box.height,
      katanaDrawioRequestSource().includes("mxgraph.cisco")
        ? renderedBottom
        : box.y + box.height,
      ...katanaDrawioTextBottomEstimates(svg),
      ...katanaDrawioHtmlTextBottomEstimates(svg),
    ),
  );
  const height = bottom - box.y;
  if (height <= box.height) {
    return;
  }
  svg.setAttribute("viewBox", `${box.x} ${box.y} ${box.width} ${height}`);
  svg.setAttribute("height", `${height}px`);
}

function katanaDrawioTextBottomEstimates(svg) {
  return Array.from(svg.querySelectorAll("text"))
    .filter(katanaDrawioTextNeedsBottomEstimate)
    .map(katanaDrawioTextBottomEstimate)
    .filter(Number.isFinite);
}

function katanaDrawioTextBottomEstimate(text) {
  return (
    katanaDrawioCoordinateAttribute(katanaDrawioNodeAttributes(text), "y") +
    katanaDrawioInheritedFontSize(text) +
    3
  );
}

function katanaDrawioTextNeedsBottomEstimate(text) {
  const group = katanaDrawioContentCellGroup(text);
  return ![
    group?.querySelector("foreignObject"),
    katanaDrawioCellHasCompleteNativeText(group),
  ].every(Boolean);
}

function katanaDrawioHtmlTextBottomEstimates(svg) {
  return Array.from(svg.querySelectorAll("foreignObject"))
    .filter(katanaDrawioHtmlTextNeedsBottomEstimate)
    .map(katanaDrawioHtmlTextBottomEstimate)
    .filter(Number.isFinite);
}

function katanaDrawioHtmlTextNeedsBottomEstimate(foreignObject) {
  const group = katanaDrawioContentCellGroup(foreignObject);
  return !katanaDrawioCellHasCompleteNativeText(group);
}

function katanaDrawioCellHasCompleteNativeText(group) {
  const id = group?.getAttribute("data-cell-id");
  const entry = katanaDrawioSourceLabelEntries().find((candidate) => candidate.id === id);
  if (!group || !entry) {
    return false;
  }
  const content = katanaDrawioCellLocalElements(group, "text")
    .map((text) => String(text.textContent ?? ""))
    .join(" ");
  return katanaDrawioTextLines(entry.label)
    .map((line) => line.trim())
    .filter(Boolean)
    .every((line) => content.includes(line));
}

function katanaDrawioHtmlTextBottomEstimate(foreignObject) {
  return (
    katanaDrawioHtmlTextOuterTop(foreignObject) +
    katanaDrawioHtmlTextContentFontSize(foreignObject) +
    3
  );
}

function katanaDrawioHtmlTextOuterTop(foreignObject) {
  return [
    ...Array.from(foreignObject.querySelectorAll("div")),
  ]
    .map((div) => katanaDrawioStylePropertyValue(String(div.getAttribute("style") ?? ""), "padding-top"))
    .filter(Boolean)
    .map(katanaDrawioCssPixels)
    .filter(Number.isFinite)
    .concat([Number.NaN])[0];
}

function katanaDrawioHtmlTextContentFontSize(foreignObject) {
  return [
    ...Array.from(foreignObject.querySelectorAll("div")),
  ]
    .filter((div) => div.getAttribute("data-katana-drawio-html-text") === "content")
    .map((div) => katanaDrawioStylePropertyValue(String(div.getAttribute("style") ?? ""), "font-size"))
    .filter(Boolean)
    .map(katanaDrawioCssPixels)
    .filter(Number.isFinite)
    .concat([Number.NaN])[0];
}

function katanaDrawioInheritedFontSize(node) {
  return [
    katanaDrawioElementCellFontSize(node),
    ...katanaDrawioElementAncestors(node).map(katanaDrawioNodeFontSize),
  ]
    .filter(Number.isFinite)
    .concat([0])[0];
}

function katanaDrawioElementCellFontSize(node) {
  return Number(katanaDrawioElementCellStyleValue(node, "fontSize"));
}

function katanaDrawioNodeFontSize(node) {
  return katanaDrawioCssPixels(node?.getAttribute?.("font-size"));
}

function katanaDrawioNodeAttributes(node) {
  return new Map(
    ["x", "y"]
      .map((name) => [name, node?.getAttribute?.(name) ?? ""])
      .filter((entry) => entry[1] !== ""),
  );
}

function katanaInstallDrawioPageBackground(svg) {
  const box = katanaDrawioSvgBox(svg);
  const color = katanaDrawioPageBackgroundColor(svg);
  if (!color) {
    return;
  }
  const rect = katanaCreateDrawioBackgroundRect(box, color);
  katanaSetDrawioPageBackgroundStyle(svg, color);
  svg.insertBefore(rect, svg.firstChild);
}

function katanaDrawioPageBackgroundColor(svg) {
  return [
    katanaDrawioStyleBackground(svg.getAttribute("style")),
    katanaDrawioRequestBackground(),
  ].filter(katanaIsVisibleDrawioBackground)[0];
}

function katanaDrawioRequestBackground() {
  return globalThis.__katanaDrawioRequest?.background ?? "";
}

function katanaDrawioStyleBackground(style) {
  const match = String(style).match(/background(?:-color)?:\s*([^;]+)/);
  return [match].filter(Boolean).map(katanaDrawioMatchValue).concat([""])[0];
}

function katanaDrawioMatchValue(match) {
  return match[1].trim();
}

function katanaIsVisibleDrawioBackground(color) {
  return !["", "transparent"].includes(String(color).toLowerCase());
}

function katanaSetDrawioPageBackgroundStyle(svg, color) {
  svg.setAttribute("style", `background: ${color}; background-color: ${color};`);
}

function katanaCreateDrawioBackgroundRect(box, color) {
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("data-katana-drawio-background", "true");
  rect.setAttribute("x", box.x);
  rect.setAttribute("y", box.y);
  rect.setAttribute("width", box.width);
  rect.setAttribute("height", box.height);
  rect.setAttribute("fill", color);
  return rect;
}

function katanaDrawioSvgBox(svg) {
  return katanaParsedDrawioViewBox(svg.getAttribute("viewBox")) ?? katanaDrawioSizeBox(svg);
}

function katanaParsedDrawioViewBox(value) {
  const parts = String(value).trim().split(/\s+/).map(Number);
  return [parts].filter(katanaIsDrawioBoxParts).map(katanaDrawioBoxFromParts).concat([null])[0];
}

function katanaIsDrawioBoxParts(parts) {
  return [parts.length === 4, parts.every(Number.isFinite)].every(Boolean);
}

function katanaDrawioBoxFromParts(parts) {
  return { x: parts[0], y: parts[1], width: parts[2], height: parts[3] };
}

function katanaDrawioSizeBox(svg) {
  return {
    x: 0,
    y: 0,
    width: katanaDrawioCssPixels(svg.getAttribute("width")),
    height: katanaDrawioCssPixels(svg.getAttribute("height")),
  };
}

function katanaDrawioCssPixels(value) {
  const pixels = Number(String(value).replace(/px$/, ""));
  return [pixels].filter(Number.isFinite).concat([0])[0];
}
