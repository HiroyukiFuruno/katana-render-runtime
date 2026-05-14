import type { MarkdownMermaidFixture } from "./split_markdown_fixtures_core";

export interface HeadingState {
  level2Index: number;
  level3Index: number;
  level2Title: string;
  level3Title: string;
}

type Level2NumberReader = (heading: HeadingState, match: RegExpMatchArray | null) => number;

type MermaidSourceReader = (source: string, lines: string[]) => string;

type MermaidFrontmatterBodyReader = (source: string, lines: string[], endOffset: number) => string;

export const MarkdownMermaidFixtureFactory = {
  from(heading: HeadingState, source: string): MarkdownMermaidFixture {
    const title = FixtureTitle.from(heading);
    return {
      fileName: FixtureFileName.from(heading, title, source),
      title,
      source,
    };
  },
};

const FixtureTitle = {
  from(heading: HeadingState): string {
    return [heading.level3Title, heading.level2Title, "Mermaid"].find((it) => it.length > 0) ?? "";
  },
};

const FixtureFileName = {
  from(heading: HeadingState, title: string, source: string): string {
    const prefix = FixtureIndex.from(heading);
    return `${prefix}-${FixtureSlug.from(title, source)}.md`;
  },
};

const FixtureIndex = {
  from(heading: HeadingState): string {
    const level2 = String(FixtureIndex.level2Number(heading)).padStart(2, "0");
    if (heading.level3Index === 0) {
      return level2;
    }
    return `${level2}-${String(heading.level3Index).padStart(2, "0")}`;
  },

  level2Number(heading: HeadingState): number {
    const match = heading.level2Title.match(/^(\d+)\./);
    return FixtureIndex.level2NumberReader(Boolean(match))(heading, match);
  },

  level2NumberReaders(): Level2NumberReader[] {
    return [FixtureIndex.fallbackLevel2Number, FixtureIndex.parsedLevel2Number];
  },

  level2NumberReader(hasMatch: boolean): Level2NumberReader {
    const reader = FixtureIndex.level2NumberReaders().at(Number(hasMatch));
    if (reader === undefined) {
      throw new Error("Mermaid fixture index reader is missing");
    }
    return reader;
  },

  fallbackLevel2Number(heading: HeadingState, _match: RegExpMatchArray | null) {
    return heading.level2Index;
  },

  parsedLevel2Number(_heading: HeadingState, match: RegExpMatchArray | null) {
    return Number.parseInt(match?.[1] ?? "0", 10);
  },
};

const FixtureSlug = {
  from(title: string, source: string): string {
    return (
      [title, DiagramKindName.from(source)]
        .map((value) => FixtureSlug.slug(value))
        .find((value) => value.length > 0) ?? "diagram"
    );
  },

  slug(value: string): string {
    return value
      .replace(/^\d+(?:\.\d+)*\.\s*/, "")
      .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");
  },
};

const DiagramKindName = {
  from(source: string): string {
    return DiagramKindName.firstToken(MermaidSource.withoutFrontmatter(source));
  },

  firstToken(source: string): string {
    const kind = DiagramKindName.rawFirstToken(source);
    return kind.length === 0 ? "empty" : kind;
  },

  rawFirstToken(source: string): string {
    return DiagramKindName.firstLine(source).trim().split(/\s+/).at(0) ?? "";
  },

  firstLine(source: string): string {
    return source.split(/\r?\n/).at(0) ?? "";
  },
};

const MermaidSource = {
  withoutFrontmatter(source: string): string {
    const lines = source.split(/\r?\n/);
    return MermaidSource.frontmatterReader(lines)(source, lines);
  },

  hasFrontmatter(lines: string[]): boolean {
    return lines[0]?.trim() === "---";
  },

  frontmatterReaders(): MermaidSourceReader[] {
    return [MermaidSource.original, MermaidSource.afterFrontmatter];
  },

  frontmatterReader(lines: string[]): MermaidSourceReader {
    const reader = MermaidSource.frontmatterReaders().at(
      Number(MermaidSource.hasFrontmatter(lines)),
    );
    if (reader === undefined) {
      throw new Error("Mermaid frontmatter reader is missing");
    }
    return reader;
  },

  original(source: string, _lines: string[]): string {
    return source;
  },

  afterFrontmatter(source: string, lines: string[]): string {
    const endIndex = lines.slice(1).findIndex((line) => line.trim() === "---");
    return MermaidSource.frontmatterBodyReader(endIndex)(source, lines, endIndex);
  },

  frontmatterBodyReaders(): MermaidFrontmatterBodyReader[] {
    return [MermaidSource.originalWithEndOffset, MermaidSource.bodyAfterFrontmatter];
  },

  frontmatterBodyReader(endIndex: number): MermaidFrontmatterBodyReader {
    const reader = MermaidSource.frontmatterBodyReaders().at(Number(endIndex >= 0));
    if (reader === undefined) {
      throw new Error("Mermaid frontmatter body reader is missing");
    }
    return reader;
  },

  originalWithEndOffset(source: string, _lines: string[], _endOffset: number) {
    return source;
  },

  bodyAfterFrontmatter(_source: string, lines: string[], endOffset: number) {
    return lines.slice(endOffset + 2).join("\n");
  },
};
