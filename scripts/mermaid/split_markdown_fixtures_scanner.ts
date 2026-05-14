import assert from "node:assert/strict";
import type { MarkdownMermaidFixture } from "./split_markdown_fixtures_core";
import {
  type HeadingState,
  MarkdownMermaidFixtureFactory,
} from "./split_markdown_fixtures_factory";

type ScannerLineHandler = (
  scanner: MarkdownMermaidScanner,
  lines: string[],
  index: number,
  line: string,
) => void;

type HeadingApplier = (heading: HeadingState, title: string) => HeadingState;

type FenceHandler = (scanner: MarkdownMermaidScanner, lines: string[], index: number) => void;

interface MermaidBlock {
  source: string;
  endIndex: number;
}

export class MarkdownMermaidScanner {
  private fixtures: MarkdownMermaidFixture[] = [];
  private skipUntilIndex = -1;
  private heading: HeadingState = {
    level2Index: 0,
    level3Index: 0,
    level2Title: "",
    level3Title: "",
  };

  constructor(private markdown: string) {}

  scan(): MarkdownMermaidFixture[] {
    this.markdown.split(/\r?\n/).forEach((line, index, lines) => {
      this.scanLine(lines, index, line);
    });
    return this.fixtures;
  }

  private scanLine(lines: string[], index: number, line: string) {
    MarkdownMermaidScanner.lineHandler(index <= this.skipUntilIndex)(this, lines, index, line);
  }

  private static lineHandlers(): ScannerLineHandler[] {
    return [MarkdownMermaidScanner.scanActiveLine, MarkdownMermaidScanner.skipLine];
  }

  private static lineHandler(isSkipped: boolean): ScannerLineHandler {
    const handler = MarkdownMermaidScanner.lineHandlers().at(Number(isSkipped));
    if (handler === undefined) {
      throw new Error("Mermaid scanner line handler is missing");
    }
    return handler;
  }

  private static scanActiveLine(
    scanner: MarkdownMermaidScanner,
    lines: string[],
    index: number,
    line: string,
  ) {
    scanner.updateHeading(line);
    scanner.scanFenceLine(lines, index, line);
  }

  private static skipLine(
    _scanner: MarkdownMermaidScanner,
    _lines: string[],
    _index: number,
    _line: string,
  ) {}

  private scanFenceLine(lines: string[], index: number, line: string) {
    MarkdownMermaidScanner.fenceHandler(FenceLine.isStart(line))(this, lines, index);
  }

  private static fenceHandlers(): FenceHandler[] {
    return [MarkdownMermaidScanner.keepLine, MarkdownMermaidScanner.pushFenceBlock];
  }

  private static fenceHandler(isStart: boolean): FenceHandler {
    const handler = MarkdownMermaidScanner.fenceHandlers().at(Number(isStart));
    if (handler === undefined) {
      throw new Error("Mermaid scanner fence handler is missing");
    }
    return handler;
  }

  private static keepLine(_scanner: MarkdownMermaidScanner, _lines: string[], _index: number) {}

  private static pushFenceBlock(scanner: MarkdownMermaidScanner, lines: string[], index: number) {
    const block = MermaidFenceBlock.read(lines, index);
    scanner.fixtures.push(MarkdownMermaidFixtureFactory.from(scanner.heading, block.source));
    scanner.skipUntilIndex = block.endIndex;
  }

  private updateHeading(line: string) {
    this.heading = HeadingLine.from(line).apply(this.heading);
  }
}

class HeadingLine {
  private constructor(
    private level: number,
    private title: string,
  ) {}

  static from(line: string): HeadingLine {
    const heading = [HeadingLine.level2(line), HeadingLine.level3(line)]
      .filter((heading) => heading.isMatched())
      .concat([HeadingLine.keep()])
      .at(0);
    if (heading === undefined) {
      throw new Error("Mermaid heading line is missing");
    }
    return heading;
  }

  private static level2(line: string): HeadingLine {
    return new HeadingLine(2, line.match(/^##\s+(.+)$/)?.[1]?.trim() ?? "");
  }

  private static level3(line: string): HeadingLine {
    return new HeadingLine(3, line.match(/^###\s+(.+)$/)?.[1]?.trim() ?? "");
  }

  private static keep(): HeadingLine {
    return new HeadingLine(0, "");
  }

  private isMatched(): boolean {
    return this.title.length > 0;
  }

  apply(heading: HeadingState): HeadingState {
    return HeadingLine.applier(this.level)(heading, this.title);
  }

  private static appliers(): HeadingApplier[] {
    return [
      HeadingLine.keepHeading,
      HeadingLine.keepHeading,
      HeadingLine.applyLevel2,
      HeadingLine.applyLevel3,
    ];
  }

  private static applier(level: number): HeadingApplier {
    const applier = HeadingLine.appliers().at(level);
    if (applier === undefined) {
      throw new Error(`Mermaid heading applier is missing: ${level}`);
    }
    return applier;
  }

  private static keepHeading(heading: HeadingState, _title: string): HeadingState {
    return heading;
  }

  private static applyLevel2(heading: HeadingState, title: string): HeadingState {
    return {
      level2Index: heading.level2Index + 1,
      level3Index: 0,
      level2Title: title,
      level3Title: "",
    };
  }

  private static applyLevel3(heading: HeadingState, title: string): HeadingState {
    return {
      ...heading,
      level3Index: heading.level3Index + 1,
      level3Title: title,
    };
  }
}

class MermaidFenceBlock {
  static read(lines: string[], startIndex: number): MermaidBlock {
    const marker = FenceLine.marker(MermaidFenceBlock.lineAt(lines, startIndex));
    const endOffset = lines.slice(startIndex + 1).findIndex((line) => line.trim() === marker);
    assert(endOffset >= 0, `Mermaid fence is not closed at line ${startIndex + 1}`);
    const endIndex = startIndex + 1 + endOffset;
    return {
      source: lines
        .slice(startIndex + 1, endIndex)
        .join("\n")
        .trim(),
      endIndex,
    };
  }

  private static lineAt(lines: string[], index: number): string {
    const line = lines.at(index);
    if (line === undefined) {
      throw new Error(`Mermaid fence line is missing: ${index}`);
    }
    return line;
  }
}

class FenceLine {
  static isStart(line: string): boolean {
    return /^(`{3,}|~{3,})mermaid\b/.test(line.trim());
  }

  static marker(line: string): string {
    const match = line.trim().match(/^(`{3,}|~{3,})/);
    assert(match, `Invalid Mermaid fence: ${line}`);
    const marker = match.at(1);
    assert(marker, `Invalid Mermaid fence marker: ${line}`);
    return marker;
  }
}
