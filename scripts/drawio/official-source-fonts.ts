import type { PageHandle } from "../mermaid/official-renderer-types";

const fontSourcePattern = /(?:^|[;'"])fontSource=([^;'"<]+)/g;

const xmlEntities: Record<string, string> = {
  amp: "&",
  apos: "'",
  gt: ">",
  lt: "<",
  quot: '"',
};

function decodeXmlEntities(source: string): string {
  return source.replace(
    /&(amp|apos|gt|lt|quot);/g,
    (entityMatch, entity: string) => xmlEntities[entity] ?? entityMatch,
  );
}

function decodeFontSource(value: string): string | null {
  try {
    const url = new URL(decodeURIComponent(value));
    return url.protocol === "https:" ? url.href : null;
  } catch {
    return null;
  }
}

export const OfficialDrawioSourceFonts = {
  urls(source: string): string[] {
    const urls = new Set<string>();
    for (const match of decodeXmlEntities(source).matchAll(fontSourcePattern)) {
      const rawUrl = match[1];
      const url = rawUrl === undefined ? null : decodeFontSource(rawUrl);
      if (url !== null) {
        urls.add(url);
      }
    }
    return [...urls];
  },

  install(page: PageHandle, source: string): Promise<void> {
    return page.evaluate(async (urls) => {
      await Promise.all(
        urls.map(
          (url) =>
            new Promise<void>((resolve, reject) => {
              const link = document.createElement("link");
              link.rel = "stylesheet";
              link.href = url;
              link.addEventListener("load", () => resolve(), { once: true });
              link.addEventListener(
                "error",
                () => reject(new Error(`failed to load Draw.io source font stylesheet: ${url}`)),
                { once: true },
              );
              document.head.appendChild(link);
            }),
        ),
      );
    }, OfficialDrawioSourceFonts.urls(source));
  },
};
