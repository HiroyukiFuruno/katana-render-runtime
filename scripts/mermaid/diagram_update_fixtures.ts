import fs from "node:fs";
import path from "node:path";
import type { RenderFixture } from "./official-renderer";

export interface Fixture extends RenderFixture {
  fileName: string;
  filePath: string;
  markdown: string;
  originalFenceSource?: string;
  canonicalSource?: string;
  fenceLanguage?: MermaidFenceLanguage;
  fenceEnd: number;
}

interface MermaidBlockResult {
  source: string;
  originalSource: string;
  canonicalSource: string;
  language: MermaidFenceLanguage;
  end: number;
}

interface MarkdownLine {
  text: string;
  separator: string;
  end: number;
}

type MermaidFenceLanguage = "mermaid" | "zenuml";

export class FixtureRepository {
  private fixturesDir: string;

  constructor(fixturesDir: string) {
    this.fixturesDir = fixturesDir;
  }

  list(): Fixture[] {
    return fs
      .readdirSync(this.fixturesDir)
      .filter((fileName) => fileName.endsWith(".md"))
      .sort()
      .map((fileName) => this.read(fileName));
  }

  read(fileName: string): Fixture {
    const filePath = path.join(this.fixturesDir, fileName);
    const markdown = fs.readFileSync(filePath, "utf8");
    const block = MermaidBlock.extract(markdown, filePath);
    return {
      fileName: fileName,
      filePath,
      slug: path.basename(fileName, ".md"),
      title: MarkdownTitle.extract(markdown, fileName),
      markdown,
      source: block.source,
      originalFenceSource: block.originalSource,
      canonicalSource: block.canonicalSource,
      fenceLanguage: block.language,
      fenceEnd: block.end,
    };
  }
}

const MermaidBlock = {
  extract(markdown: string, filePath: string): MermaidBlockResult {
    let language: MermaidFenceLanguage | undefined;
    const content: MarkdownLine[] = [];
    for (const line of markdownLines(markdown)) {
      if (language === undefined) {
        language = MermaidBlock.openingLanguage(line.text);
        continue;
      }
      if (MermaidBlock.isClosing(line.text)) {
        const canonicalBody = content.map((entry) => entry.text).join("\n");
        const originalSource = content
          .map(
            (entry, index) => `${entry.text}${index + 1 < content.length ? entry.separator : ""}`,
          )
          .join("");
        return {
          source: MermaidBlock.source(language, originalSource),
          originalSource,
          canonicalSource: language === "zenuml" ? `zenuml\n${canonicalBody}` : canonicalBody,
          language,
          end: line.end,
        };
      }
      content.push(line);
    }
    throw new Error(`Mermaid block not found: ${filePath}`);
  },

  source(language: MermaidFenceLanguage, originalSource: string): string {
    const trimmed = originalSource.trim();
    return language === "zenuml" ? `zenuml\n${trimmed}` : trimmed;
  },

  openingLanguage(line: string): MermaidFenceLanguage | undefined {
    const trimmed = line.trim();
    const marker = trimmed[0];
    if (marker !== "`" && marker !== "~") return undefined;
    const markerCount = trimmed.match(marker === "`" ? /^`+/ : /^~+/)?.[0].length ?? 0;
    if (markerCount < 3) return undefined;
    const language = trimmed.slice(markerCount).trim().split(/\s+/)[0];
    return language === "mermaid" || language === "zenuml" ? language : undefined;
  },

  isClosing(line: string): boolean {
    const trimmed = line.trim();
    return trimmed === "```" || trimmed === "~~~";
  },
};

function markdownLines(markdown: string): MarkdownLine[] {
  const result: MarkdownLine[] = [];
  let start = 0;
  while (start < markdown.length) {
    const newline = markdown.indexOf("\n", start);
    const rawEnd = newline < 0 ? markdown.length : newline;
    const raw = markdown.slice(start, rawEnd);
    const carriageReturn = newline >= 0 && raw.endsWith("\r");
    result.push({
      text: carriageReturn ? raw.slice(0, -1) : raw,
      separator: newline < 0 ? "" : carriageReturn ? "\r\n" : "\n",
      end: newline < 0 ? rawEnd : newline + 1,
    });
    if (newline < 0) break;
    start = newline + 1;
  }
  return result;
}

const MarkdownTitle = {
  extract(markdown: string, fileName: string): string {
    return markdown.match(/^#\s+(.+)$/m)?.[1] || path.basename(fileName, ".md");
  },
};
