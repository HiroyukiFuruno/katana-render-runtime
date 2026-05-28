import { parse } from "acorn";

interface AcornIdentifier {
  readonly name: string;
}

interface AcornStatement {
  readonly type: string;
  readonly start: number;
  readonly end: number;
}

interface AcornFunctionDeclaration extends AcornStatement {
  readonly type: "FunctionDeclaration";
  readonly id: AcornIdentifier | null;
}

interface AcornProgram {
  readonly type: "Program";
  readonly body: readonly AcornStatement[];
}

interface SourceRange {
  readonly start: number;
  readonly end: number;
}

export class RuntimeScriptNormalizer {
  deduplicateFunctionDeclarations(source: string): string {
    const program = parse(source, {
      ecmaVersion: "latest",
      sourceType: "script",
    }) as AcornProgram;
    return this.removeRanges(source, this.duplicateFunctionRanges(program));
  }

  private duplicateFunctionRanges(program: AcornProgram): SourceRange[] {
    const functions = new Map<string, AcornFunctionDeclaration[]>();
    for (const statement of program.body) {
      if (!this.isFunctionDeclaration(statement) || statement.id === null) {
        continue;
      }
      const entries = functions.get(statement.id.name) ?? [];
      entries.push(statement);
      functions.set(statement.id.name, entries);
    }
    const ranges: SourceRange[] = [];
    for (const entries of functions.values()) {
      ranges.push(...entries.slice(0, -1).map((entry) => this.removalRange(entry)));
    }
    return ranges;
  }

  private isFunctionDeclaration(statement: AcornStatement): statement is AcornFunctionDeclaration {
    return statement.type === "FunctionDeclaration";
  }

  private removalRange(statement: AcornFunctionDeclaration): SourceRange {
    return {
      start: statement.start,
      end: statement.end,
    };
  }

  private removeRanges(source: string, ranges: SourceRange[]): string {
    let output = source;
    const sortedRanges = [...ranges].sort((left, right) => right.start - left.start);
    for (const range of sortedRanges) {
      output = `${output.slice(0, range.start)}\n${output.slice(range.end)}`;
    }
    return output;
  }
}
