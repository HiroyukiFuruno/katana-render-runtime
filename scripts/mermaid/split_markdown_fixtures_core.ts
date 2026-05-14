import { MarkdownMermaidScanner } from "./split_markdown_fixtures_scanner";

export interface MarkdownMermaidFixture {
  fileName: string;
  title: string;
  source: string;
}

export const MarkdownMermaidFixtures = {
  from(markdown: string): MarkdownMermaidFixture[] {
    return new MarkdownMermaidScanner(markdown).scan();
  },
};
