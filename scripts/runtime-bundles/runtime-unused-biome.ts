import { z } from "zod";
import { unusedDiagnosticSchema } from "./runtime-unused-origin";

const reportSchema = z.object({
  summary: z.object({
    errors: z.number().int().nonnegative(),
    warnings: z.number().int().nonnegative(),
    diagnosticsNotPrinted: z.number().int().nonnegative(),
  }),
  diagnostics: z.array(unusedDiagnosticSchema),
});
const unusedCategories = new Set([
  "lint/correctness/noUnusedVariables",
  "lint/correctness/noUnusedFunctionParameters",
]);

export function inspectUnusedBiomeReport(value: unknown, exitCode: number) {
  const report = reportSchema.parse(value);
  if (
    report.summary.diagnosticsNotPrinted !== 0 ||
    report.summary.errors !== 0 ||
    ![0, 1].includes(exitCode)
  )
    throw new Error("incomplete/failed Biome report");
  if (
    report.diagnostics.some(
      (diagnostic) =>
        diagnostic.severity !== "warning" || !unusedCategories.has(diagnostic.category),
    )
  )
    throw new Error("unknown Biome diagnostic");
  if (
    report.summary.warnings !== report.diagnostics.length ||
    (report.summary.warnings === 0) !== (exitCode === 0)
  )
    throw new Error("Biome summary/exit mismatch");
  return report.diagnostics;
}

export function inspectUnusedBiomeDiagnostics(value: unknown, exitCode: number) {
  const diagnostics = z.array(unusedDiagnosticSchema).parse(value);
  return inspectUnusedBiomeReport(
    {
      summary: { warnings: diagnostics.length, errors: 0, diagnosticsNotPrinted: 0 },
      diagnostics,
    },
    exitCode,
  );
}
