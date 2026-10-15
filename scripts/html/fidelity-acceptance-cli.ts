import { z } from "zod";
import { runDynamicFidelity } from "./dynamic-fidelity";
import { assertFeatureResultReportsComplete, loadFeatureCatalog } from "./feature-catalog";
import { assertInterimFeatureResultReportsComplete } from "./interim-feature-results";
import { tocBinaryFromBuildReport } from "./toc-fidelity";
import { runInputStateFidelity } from "./visual-state-fidelity";

const modeSchema = z.enum([
  "input-state-final",
  "input-state-interim",
  "dynamic-final",
  "dynamic-interim",
  "reports-final",
  "reports-interim",
]);

type CaptureMode = Exclude<z.infer<typeof modeSchema>, "reports-final" | "reports-interim">;

interface CapturePlan {
  kind: "input-state" | "dynamic";
  buildReport: string;
  output: string;
  threshold: 99 | 90;
  options?: { phase: "interim" };
}

interface ReportsPlan {
  kind: "reports";
  phase: "final" | "interim";
  visualReport: string;
  scriptReport: string;
}

export type FidelityAcceptancePlan = CapturePlan | ReportsPlan;

const capturePlans: Record<CaptureMode, CapturePlan> = {
  "input-state-final": {
    kind: "input-state",
    buildReport: "tmp/html-fidelity/input-state/test-build.jsonl",
    output: "tmp/html-fidelity/input-state",
    threshold: 99,
  },
  "input-state-interim": {
    kind: "input-state",
    buildReport: "tmp/html-fidelity/input-state-interim/test-build.jsonl",
    output: "tmp/html-fidelity/input-state-interim",
    threshold: 90,
    options: { phase: "interim" },
  },
  "dynamic-final": {
    kind: "dynamic",
    buildReport: "tmp/html-fidelity/dynamic/test-build.jsonl",
    output: "tmp/html-fidelity/dynamic",
    threshold: 99,
  },
  "dynamic-interim": {
    kind: "dynamic",
    buildReport: "tmp/html-fidelity/dynamic-interim/test-build.jsonl",
    output: "tmp/html-fidelity/dynamic-interim",
    threshold: 90,
    options: { phase: "interim" },
  },
};

export function fidelityAcceptancePlan(argv: readonly string[]): FidelityAcceptancePlan {
  const mode = modeSchema.parse(argv[0]);
  if (mode === "reports-final" || mode === "reports-interim") {
    if (argv.length !== 1) throw new Error("report acceptance mode takes no additional arguments");
    return mode === "reports-final"
      ? {
          kind: "reports",
          phase: "final",
          visualReport: "tmp/html-fidelity/visual-feature-results.json",
          scriptReport: "tmp/html-fidelity/dynamic/script-feature-results.json",
        }
      : {
          kind: "reports",
          phase: "interim",
          visualReport: "tmp/html-fidelity/interim/interim-visual-feature-results.json",
          scriptReport: "tmp/html-fidelity/dynamic-interim/interim-script-feature-results.json",
        };
  }
  const plan = capturePlans[mode];
  if (argv.length !== 2 || argv[1] !== plan.buildReport)
    throw new Error("capture acceptance mode requires its fixed build report argument");
  return plan.options ? { ...plan, options: { ...plan.options } } : { ...plan };
}

export async function runFidelityAcceptance(argv: readonly string[]): Promise<void> {
  const plan = fidelityAcceptancePlan(argv);
  if (plan.kind === "reports") {
    const catalog = loadFeatureCatalog();
    if (plan.phase === "final")
      assertFeatureResultReportsComplete(catalog, plan.visualReport, plan.scriptReport);
    else assertInterimFeatureResultReportsComplete(catalog, plan.visualReport, plan.scriptReport);
    return;
  }
  const binary = tocBinaryFromBuildReport(plan.buildReport);
  const capture = plan.kind === "input-state" ? runInputStateFidelity : runDynamicFidelity;
  if (plan.options) await capture(binary, plan.output, plan.threshold, plan.options);
  else await capture(binary, plan.output, plan.threshold);
}

if (import.meta.main) await runFidelityAcceptance(process.argv.slice(2));
