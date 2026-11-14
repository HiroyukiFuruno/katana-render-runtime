export type FidelityPhase = "final" | "interim";

export function fidelityThresholdForPhase(
  phase: FidelityPhase,
  requestedThreshold: number,
): 90 | 99 {
  if (phase !== "final" && phase !== "interim") {
    throw new Error(`unsupported HTML fidelity phase: ${String(phase)}`);
  }
  const requiredThreshold = phase === "interim" ? 90 : 99;
  if (requestedThreshold !== requiredThreshold) {
    throw new Error(`HTML ${phase} fidelity requires the immutable ${requiredThreshold} threshold`);
  }
  return requiredThreshold;
}
