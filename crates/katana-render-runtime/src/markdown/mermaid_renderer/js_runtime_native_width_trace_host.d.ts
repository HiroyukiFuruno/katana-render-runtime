interface NativeWidthSvgMetrics {
  advance: number;
  ascent: number;
  descent: number;
  layoutAscent?: number;
  inkLeft?: number;
  inkTop?: number;
  inkRight?: number;
  inkBottom?: number;
}
interface NativeWidthTraceCall {
  kind: "css" | "canvas" | "svg";
  args: unknown[];
  width: number | undefined;
  // 未指定と、指定されたが契約外の値(null)を区別して記録する。
  segments?: string[] | null;
  metrics?: NativeWidthSvgMetrics;
}
interface NativeWidthTraceBox {
  x: number;
  y: number;
  width: number;
  height: number;
}
interface NativeWidthTraceNodeSnapshot {
  localName: string;
  namespaceURI: string | null;
  id: string | null;
  className: string | null;
  text: string;
  childSvgWidths: Array<string | null>;
}
interface NativeWidthTraceBBoxObservation {
  node: NativeWidthTraceNodeSnapshot;
  parent: NativeWidthTraceNodeSnapshot | null;
  bbox: NativeWidthTraceBox;
  children: Array<{
    node: NativeWidthTraceNodeSnapshot;
    transform: string | null;
    bbox: NativeWidthTraceBox;
  }>;
}
interface NativeWidthTextRun {
  text: string;
  x: number;
  width: number;
  fragmentTexts?: string[];
  fragmentAdvances?: number[];
  font: {
    family: string;
    size: number;
    weight: number;
    italic: boolean;
    nativeAdvanceEligible: boolean;
  };
}
interface NativeWidthTextLine {
  text: string;
  x: number;
  anchor: string;
  parts: NativeWidthTextRun[];
}
interface NativeWidthC4DescriptionRow {
  text: string;
  bbox: NativeWidthTraceBox;
  wholeAdvance: number | null;
  grouped: boolean;
  parts: Array<{
    text: string;
    x: number;
    sourceWidth: number;
    advance: number;
    family: string;
    fontSize: number;
    weight: number;
  }>;
}
interface NativeWidthTraceNode {
  namespaceURI: string | null;
  localName: string;
  id: string;
  className: string;
  textContent: string | null;
  parentNode: NativeWidthTraceNode | null;
  children: NativeWidthTraceNode[];
  style: {
    setProperty(name: unknown, value: unknown, priority?: unknown): undefined;
  };
  getAttribute(name: string): string | null;
  querySelector(selector: string): NativeWidthTraceNode | null;
  appendChild(child: NativeWidthTraceNode): NativeWidthTraceNode;
  getBBox(...args: unknown[]): NativeWidthTraceBox;
}
interface NativeWidthTraceDocument {
  documentElement: NativeWidthTraceNode;
  createElementNS(namespaceURI: string | null, tagName: string): NativeWidthTraceNode;
}

// 実Rust callbackはCSS失敗時undefined、SVG失敗時throw、成功時3数値を返す。
declare var __katanaMeasureText: (...args: unknown[]) => number | undefined;
declare var __katanaMeasureCanvasText: (...args: unknown[]) => number | undefined;
declare var __katanaMeasureSvgTextMetrics: (...args: unknown[]) => NativeWidthSvgMetrics;
declare var __katanaMermaidDiagramType: string | undefined;
declare var __krrActualWidths: NativeWidthTraceCall[];
declare var __krrActualBBoxes: NativeWidthTraceBBoxObservation[];
declare var __krrC4DescriptionRows: NativeWidthC4DescriptionRow[];
declare var __krrActualBBoxTraceOverflow: boolean;
declare var __krrActualBBoxTraceFailure: string | null;
declare var __krrTraceTargets: Array<[string, string]>;
declare var __krrRenderedSvg: string;
declare var __krrProbeWidths: NativeWidthTraceCall[];
declare var __krrBridgeCalls: number;
// traceが使う実KatanaNodeだけを投影し、標準Chrome DOMの契約は導入しない。
declare const document: NativeWidthTraceDocument;
declare function katanaOrderedTextLines(node: NativeWidthTraceNode): NativeWidthTextLine[];
declare function katanaRunLayoutAdvance(run: NativeWidthTextRun): number;
declare function katanaSvgAdvance(value: number): number;
declare function katanaAnchoredTextLeft(anchor: string, x: number, width: number): number;
declare function katanaNativeSvgRunMetrics(
  node: NativeWidthTraceNode,
  text: string,
  font: NativeWidthTextRun["font"],
): NativeWidthSvgMetrics | null;
