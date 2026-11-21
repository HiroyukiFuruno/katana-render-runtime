import {
  type ClassReferenceIntegrity,
  type ClassReferenceLanguage,
  inspectClassReference,
} from "./class-reference-integrity";

export const CLASS_REFERENCE_TARGETS: ReadonlyArray<{
  slug: string;
  language: ClassReferenceLanguage;
}> = [
  { slug: "03-02-class-diagram", language: "ja" },
  { slug: "03-02-class-diagram-inheritance", language: "en" },
];

export interface ClassReferenceGateResult {
  slug: string;
  language: ClassReferenceLanguage;
  scope: "class-inheritance-text-anchors";
  report: ClassReferenceIntegrity;
}

interface GeneratedClassReferenceInput {
  slug: string;
  diagramType: string;
  source: string;
  renderSource: string;
  rawSvg: string;
  restoredSvg: string;
}

export function validateGeneratedClassReference(
  input: GeneratedClassReferenceInput,
): ClassReferenceGateResult | null {
  const target = targetForSlug(input.slug);
  if (target === null) return null;
  if (input.diagramType.toLowerCase() !== "classdiagram") {
    throw new Error(
      `class reference gate rejected ${input.slug}: expected classDiagram source type`,
    );
  }
  if (input.source !== input.renderSource) {
    throw new Error(
      `class reference gate rejected ${input.slug}: render source differs from fixture`,
    );
  }
  if (input.rawSvg !== input.restoredSvg) {
    throw new Error(`class reference gate rejected ${input.slug}: raw/restored SVG differ`);
  }
  const report = inspectClassReference(input.rawSvg, target.language);
  if (!report.accepted) throw rejected(input.slug, "generated SVG", report.reasons);
  return { slug: input.slug, language: target.language, scope: report.scope, report };
}

export function validateScoredClassReferencePair(
  slug: string,
  officialSvg: string | null,
  candidateSvg: string | null,
): ClassReferenceGateResult | null {
  const target = targetForSlug(slug);
  if (target === null) return null;
  if (officialSvg === null) {
    throw new Error(`class reference gate rejected ${slug}: missing official SVG`);
  }
  if (candidateSvg === null) {
    throw new Error(`class reference gate rejected ${slug}: missing candidate SVG`);
  }

  const official = inspectClassReference(officialSvg, target.language);
  const candidate = inspectClassReference(candidateSvg, target.language);
  const failures = [
    ...(!official.accepted ? [`official: ${official.reasons.join(", ")}`] : []),
    ...(!candidate.accepted ? [`candidate: ${candidate.reasons.join(", ")}`] : []),
  ];
  if (failures.length > 0) {
    throw new Error(`class reference gate rejected ${slug}: ${failures.join("; ")}`);
  }
  return {
    slug,
    language: target.language,
    scope: official.scope,
    report: official,
  };
}

export function isClassReferenceTarget(slug: string): boolean {
  return targetForSlug(slug) !== null;
}

function targetForSlug(slug: string) {
  return CLASS_REFERENCE_TARGETS.find((target) => target.slug === slug) ?? null;
}

function rejected(slug: string, artifact: string, reasons: string[]): Error {
  return new Error(`class reference gate rejected ${slug} ${artifact}: ${reasons.join(", ")}`);
}
