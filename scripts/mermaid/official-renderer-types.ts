import type { MermaidThemeName, MermaidThemeVariables } from "./diagram_theme";
import type { MermaidZenumlBrowserGlobals } from "./official-renderer-browser-globals.d.ts";
import type { JsonInput } from "./raster-json-input";

export interface BrowserLauncher {
  launch(options: { headless: boolean }): Promise<BrowserHandle>;
  executablePath?(): string;
}

export interface PlaywrightModule {
  chromium: BrowserLauncher;
}

export interface BrowserIdentityCDPSession {
  send(method: "SystemInfo.getProcessInfo"): Promise<JsonInput>;
  detach(): Promise<void>;
}

export interface BrowserHandle {
  newBrowserCDPSession?(): Promise<BrowserIdentityCDPSession>;
  version?(): string;
  newPage(options: {
    viewport: { width: number; height: number };
    deviceScaleFactor: number;
  }): Promise<PageHandle>;
  close(): Promise<void>;
}

export interface PageHandle {
  setContent(html: string, options: { waitUntil: "load" }): Promise<void>;
  setViewportSize?(viewport: { width: number; height: number }): Promise<void>;
  addScriptTag(options: { path: string }): Promise<undefined | object>;
  evaluate<Result>(callback: () => Result | Promise<Result>): Promise<Result>;
  evaluate<Result, Argument>(
    callback: (argument: Argument) => Result | Promise<Result>,
    argument: Argument,
  ): Promise<Result>;
  locator(selector: string): LocatorHandle;
  close(): Promise<void>;
}

export interface LocatorHandle {
  screenshot(options: { path: string; omitBackground: boolean }): Promise<Buffer>;
}

export interface MermaidConfig {
  startOnLoad: boolean;
  securityLevel: "loose";
  htmlLabels: boolean;
  theme: MermaidThemeName;
  flowchart: { htmlLabels: boolean; useMaxWidth: boolean };
  sequence: { useMaxWidth: boolean };
  themeVariables: MermaidThemeVariables;
}

export interface MermaidRenderResult {
  svg: string;
  diagramType: string;
}

export interface MermaidSvgStages {
  diagramType: string;
  rawSvg: string;
  restoredSvg: string;
  normalizedSvg: string;
  originalSource: string;
  preprocessedSource: string;
  renderSource: string;
  replacements: MermaidI18nReplacement[];
  mermaidVersion: string | null;
}

export interface MermaidGlobal {
  version?: string;
  initialize(config: MermaidConfig): void;
  registerExternalDiagrams(diagrams: object[]): Promise<void>;
  detectType(source: string): string;
  render(id: string, source: string): Promise<MermaidRenderResult>;
}

export interface MermaidWindow extends Window {
  mermaid?: MermaidGlobal;
}

export interface MermaidI18nReplacement {
  placeholder: string;
  text: string;
}

export interface MermaidI18nNormalizeResult {
  source: string;
  replacements: MermaidI18nReplacement[];
}

export interface MermaidI18nWindow extends MermaidWindow, MermaidZenumlBrowserGlobals {
  __katanaOfficialReference?: boolean;
  __katanaTransformSvgBox?: (
    transform: string,
    x: number,
    y: number,
    width: number,
    height: number,
  ) => { x: number; y: number; width: number; height: number };
  katanaMermaidDiagramType?: (source: string) => string;
  katanaNormalizeMermaidSourceI18n?: (source: string) => MermaidI18nNormalizeResult;
  katanaRestoreMermaidI18nText?: (svg: string, replacements: MermaidI18nReplacement[]) => string;
  katanaNormalizeMermaidSvg?: (
    svg: string,
    request: {
      theme: "dark" | "default";
      text: string;
      fill: string;
      stroke: string;
      arrow: string;
      source: string;
      background: string;
    },
  ) => string;
}

export interface FontReadyDocument extends Document {
  fonts: FontFaceSet;
}
