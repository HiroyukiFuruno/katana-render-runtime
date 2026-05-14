import path from "node:path";
import { DiagramTheme, type DiagramThemeName } from "./diagram_theme";

export interface CliParsedOptions {
  officialDir: string;
  katanaDir: string;
  outputDir: string;
  katanaCrop: CropRect | null;
  baselineScoresPath: string | null;
  minScore: number;
  theme: DiagramThemeName;
}

export const CliOptions = {
  parse(argv: string[]): CliParsedOptions {
    return {
      officialDir: path.resolve(
        CliOptions.get(argv, "--official", "tmp/mermaid-parts-en-dark-official-browser"),
      ),
      katanaDir: path.resolve(
        CliOptions.get(argv, "--katana", "tmp/mermaid-parts-en-dark-katana-browser"),
      ),
      outputDir: path.resolve(
        CliOptions.get(argv, "--output", "tmp/mermaid-parts-en-dark-comparison"),
      ),
      katanaCrop: CropRect.parseOptional(CliOptions.get(argv, "--katana-crop", "none")),
      baselineScoresPath: CliOptions.getOptionalPath(argv, "--baseline"),
      minScore: CliOptions.number(argv, "--min-score", 99),
      theme: DiagramTheme.parse(CliOptions.get(argv, "--theme", "dark")).name,
    };
  },

  get(argv: string[], name: string, fallback: string): string {
    const value = CliOptions.getValue(argv, name);
    return value === null ? fallback : value;
  },

  getOptionalPath(argv: string[], name: string): string | null {
    return CliOptions.getValue(argv, name);
  },

  getValue(argv: string[], name: string): string | null {
    const index = argv.indexOf(name);
    if (index < 0) {
      return null;
    }
    return argv.at(index + 1) ?? null;
  },

  number(argv: string[], name: string, fallback: number): number {
    const value = Number(CliOptions.get(argv, name, String(fallback)));
    if (!Number.isFinite(value)) {
      throw new Error(`Invalid number option: ${name}`);
    }
    return value;
  },
};

export class CropRect {
  static parseOptional(value: string): CropRect | null {
    return value === "none" ? null : CropRect.parse(value);
  }

  static parse(value: string): CropRect {
    const parts = value.split(",").map((it) => Number.parseInt(it, 10));
    if (CropRect.isInvalidParts(parts)) {
      throw new Error(`Invalid crop rect: ${value}`);
    }
    return new CropRect(
      CropRect.partAt(parts, 0),
      CropRect.partAt(parts, 1),
      CropRect.partAt(parts, 2),
      CropRect.partAt(parts, 3),
    );
  }

  private static isInvalidParts(parts: number[]): boolean {
    return [parts.length !== 4, parts.some((it) => Number.isNaN(it))].includes(true);
  }

  private static partAt(parts: number[], index: number): number {
    const value = parts.at(index);
    if (value === undefined) {
      throw new Error("Invalid crop rect part count");
    }
    return value;
  }

  constructor(
    public x: number,
    public y: number,
    public width: number,
    public height: number,
  ) {}

  toMagickArg(): string {
    return `${this.width}x${this.height}+${this.x}+${this.y}`;
  }
}
