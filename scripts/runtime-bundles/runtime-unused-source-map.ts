import { type RawSourceMap, SourceMapConsumer } from "source-map-js";
import { z } from "zod";

export interface Link {
  source: string;
  offset: number;
}

export const sourceMapSchema = z.object({
  version: z.literal(3),
  mappings: z.string(),
  sources: z.array(z.string()),
  sourcesContent: z.array(z.string()),
  names: z.array(z.string()),
  file: z.string().optional(),
  sourceRoot: z.string().optional(),
});

export class Lines {
  private readonly starts = [0];
  constructor(readonly code: string) {
    if (/\r(?!\n)|[\u2028\u2029]/u.test(code)) throw new Error("unsupported line terminator");
    for (const match of code.matchAll(/\n/g)) this.starts.push(match.index + 1);
  }
  offset(line: number, column: number): number {
    const start = this.starts[line - 1];
    const next = this.starts[line];
    if (!Number.isInteger(line) || !Number.isInteger(column) || start === undefined || column < 0)
      throw new Error("invalid source position");
    const end = next === undefined ? this.code.length : next - 1;
    if (start + column > end) throw new Error("source column out of bounds");
    return start + column;
  }
  biome(line: number, column: number): number {
    const start = this.offset(line, 0);
    const end = this.starts[line] === undefined ? this.code.length : (this.starts[line] ?? 0) - 1;
    const text = this.code.slice(start, end).replace(/\r$/, "");
    const points = Array.from(text);
    if (!Number.isInteger(column) || column < 1 || column > points.length + 1)
      throw new Error("invalid Biome column");
    return start + points.slice(0, column - 1).join("").length;
  }
}

export function mapIndex(
  value: z.infer<typeof sourceMapSchema>,
  generated: Lines,
  sources: ReadonlyMap<string, Lines>,
): Map<number, Link | null> {
  if (value.sourceRoot || value.sources.length !== value.sourcesContent.length)
    throw new Error("invalid map sources");
  for (const [index, name] of value.sources.entries())
    if (sources.get(name)?.code !== value.sourcesContent[index])
      throw new Error(`unknown/changed map source: ${name}`);
  // 実V3のversion:numberを検査し、外部宣言のversion:stringだけを境界で補う。
  const consumer = new SourceMapConsumer(value as unknown as RawSourceMap);
  const table = new Map<number, Link | null>();
  consumer.eachMapping((mapping) => {
    const at = generated.offset(mapping.generatedLine, mapping.generatedColumn);
    if (table.has(at)) throw new Error("ambiguous exact mapping");
    if (
      mapping.source === null ||
      mapping.originalLine === null ||
      mapping.originalColumn === null
    ) {
      table.set(at, null);
      return;
    }
    const source = sources.get(mapping.source);
    if (!source) throw new Error("unknown exact mapping source");
    table.set(at, {
      source: mapping.source,
      offset: source.offset(mapping.originalLine, mapping.originalColumn),
    });
  });
  return table;
}
