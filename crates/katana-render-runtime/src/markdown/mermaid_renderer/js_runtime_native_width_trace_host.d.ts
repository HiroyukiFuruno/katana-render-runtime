interface NativeWidthSvgMetrics {
  advance: number;
  ascent: number;
  descent: number;
}
interface NativeWidthTraceCall {
  kind: "css" | "svg";
  args: unknown[];
  width: number | undefined;
  metrics?: NativeWidthSvgMetrics;
}
interface NativeWidthTraceNode {
  namespaceURI: string | null;
  textContent: string;
  style: {
    setProperty(name: unknown, value: unknown, priority?: unknown): undefined;
  };
  appendChild(child: NativeWidthTraceNode): NativeWidthTraceNode;
  getBBox(): { width: number };
}
interface NativeWidthTraceDocument {
  documentElement: NativeWidthTraceNode;
  createElementNS(namespaceURI: string | null, tagName: string): NativeWidthTraceNode;
}

// 実Rust callbackはCSS失敗時undefined、SVG失敗時throw、成功時3数値を返す。
declare var __katanaMeasureText: (...args: unknown[]) => number | undefined;
declare var __katanaMeasureSvgTextMetrics: (...args: unknown[]) => NativeWidthSvgMetrics;
declare var __katanaMermaidDiagramType: string | undefined;
declare var __krrActualWidths: NativeWidthTraceCall[];
declare var __krrProbeWidths: NativeWidthTraceCall[];
declare var __krrBridgeCalls: number;
// traceが使う実KatanaNodeだけを投影し、標準Chrome DOMの契約は導入しない。
declare const document: NativeWidthTraceDocument;
