import { parseSync } from "@swc/core";
import { parseExpressionAt } from "acorn";
import { type RawSourceMap, SourceMapConsumer } from "source-map-js";
import { z } from "zod";
import { runtimeBundleChecksum } from "./runtime-bundle-checksum";
import {
  type ClassicGenerationEvidence,
  type ClassicGenerationMappedSource,
  type ClassicRollupGeneration,
  classicCompletedGenerationEvidenceSchema,
  classicRollupGenerationSchema,
} from "./runtime-bundle-types";
import { astObject, astSpan, browserAstNodes } from "./runtime-unused-origin";
import { Lines, sourceMapSchema } from "./runtime-unused-source-map";

export function classicGenerationSource(code: string, map?: string) {
  const source = { code, sha256: runtimeBundleChecksum(code) };
  return Object.freeze(map === undefined ? source : { ...source, map });
}
export function classicEvalLiteral(code: string, isModule = false): string {
  const script = isModule
    ? parseSync(code, { syntax: "ecmascript" })
    : parseSync(code, { syntax: "ecmascript", isModule: false });
  const nodes = browserAstNodes(script);
  const members = nodes.filter(
    ({ node }) =>
      node.type === "MemberExpression" &&
      astObject.parse(node.object).value === "globalThis" &&
      astObject.parse(node.property).value === "eval",
  );
  if (members.length !== 1) throw new Error("classic generation eval member count differs");
  const member = members[0]?.node;
  if (
    !member ||
    astObject.parse(member.property).type !== "Identifier" ||
    astObject.parse(member.object).ctxt !== 1
  )
    throw new Error("classic generation eval receiver differs");
  for (const { node } of nodes) {
    if (node.type === "Identifier" && node.value === "globalThis" && node.ctxt !== 1)
      throw new Error("classic generation globalThis shadowed");
    if (node.type === "AssignmentExpression" && astObject.parse(node.left).value === "globalThis")
      throw new Error("classic generation globalThis rebound");
  }
  const call = nodes.find(
    ({ node }) =>
      node.type === "CallExpression" && JSON.stringify(node.callee) === JSON.stringify(member),
  )?.node;
  if (!call) throw new Error("classic generation eval escaped");
  const args = z
    .array(
      z.object({
        spread: z.null(),
        expression: z.object({
          type: z.literal("StringLiteral"),
          value: z.string(),
          raw: z.string(),
          span: astSpan,
        }),
      }),
    )
    .parse(call.arguments);
  if (args.length !== 1 || !args[0]) throw new Error("classic generation eval argument differs");
  const literal = args[0].expression;
  const bytes = Buffer.from(code).subarray(literal.span.start - 1, literal.span.end - 1);
  if (!bytes.equals(Buffer.from(literal.raw)))
    throw new Error("classic generation literal raw/span differs");
  // SWCが保持するunicode escapeのraw値を、同じ物理literalのAcorn非実行decodeで照合する。
  const decoded = z
    .object({
      type: z.literal("Literal"),
      value: z.string(),
      start: z.literal(0),
      end: z.literal(literal.raw.length),
    })
    .parse(parseExpressionAt(literal.raw, 0, { ecmaVersion: "latest" }));
  return decoded.value;
}
export function parsedClassicGenerationMap(source: ClassicGenerationMappedSource) {
  const map = sourceMapSchema.parse(JSON.parse(source.map));
  if (
    map.sourceRoot ||
    map.sources.length !== map.sourcesContent.length ||
    new Set(map.sources).size !== map.sources.length
  )
    throw new Error("classic generation map source inventory differs");
  const originals = new Map(
    map.sources.map((name, index) => [name, new Lines(map.sourcesContent[index] ?? "")]),
  );
  const lines = new Lines(source.code);
  const consumer = new SourceMapConsumer(map as unknown as RawSourceMap);
  const positions = new Set<string>();
  consumer.eachMapping((item) => {
    lines.offset(item.generatedLine, item.generatedColumn);
    const key = `${item.generatedLine}:${item.generatedColumn}`;
    if (positions.has(key)) throw new Error("classic generation map anchor ambiguous");
    positions.add(key);
    if (item.source !== null && item.originalLine !== null && item.originalColumn !== null) {
      const original = originals.get(item.source);
      if (!original) throw new Error("classic generation original source missing");
      original.offset(item.originalLine, item.originalColumn);
    }
  });
  return map;
}
export function verifyClassicRollupGeneration(
  value: ClassicRollupGeneration,
): ClassicRollupGeneration {
  const generation = classicRollupGenerationSchema.parse(value);
  for (const source of [
    generation.normalized,
    generation.inner,
    generation.wrapper,
    generation.rollup,
    generation.final,
  ]) {
    if (source.sha256 !== runtimeBundleChecksum(source.code))
      throw new Error("classic generation source hash differs");
  }
  const inner = parsedClassicGenerationMap(generation.inner),
    outer = parsedClassicGenerationMap(generation.rollup),
    final = parsedClassicGenerationMap(generation.final);
  if (
    inner.sourcesContent.length !== 1 ||
    inner.sourcesContent[0] !== generation.normalized.code ||
    !outer.sourcesContent.includes(generation.wrapper.code)
  )
    throw new Error("classic generation map input differs");
  if (final.sources.length !== outer.sources.length)
    throw new Error("classic generation final source inventory differs");
  for (const [index, name] of final.sources.entries()) {
    const previous = outer.sources.indexOf(name);
    if (previous < 0 || final.sourcesContent[index] !== outer.sourcesContent[previous])
      throw new Error("classic generation final map input differs");
  }
  for (const source of [generation.wrapper, generation.rollup, generation.final]) {
    const decoded = classicEvalLiteral(source.code, source === generation.wrapper);
    if (!Buffer.from(decoded).equals(Buffer.from(generation.inner.code)))
      throw new Error("classic generation decoded eval bytes differ");
  }
  const provenance = z
    .object({ code: z.string() })
    .parse(JSON.parse(generation.normalizedProvenance));
  if (provenance.code !== generation.normalized.code)
    throw new Error("classic generation normalized provenance differs");
  return Object.freeze({
    ...generation,
    normalized: Object.freeze(generation.normalized),
    inner: Object.freeze(generation.inner),
    wrapper: Object.freeze(generation.wrapper),
    rollup: Object.freeze(generation.rollup),
    final: Object.freeze(generation.final),
  });
}
export async function completeClassicGeneration(
  generation: ClassicRollupGeneration,
  header: string,
  content: string,
  file: string,
): Promise<ClassicGenerationEvidence> {
  const verified = verifyClassicRollupGeneration(generation);
  const expectedHeader = `// @generated by scripts/runtime-bundles/bundle-runtime.ts\n// bundle: ${verified.name}\n`;
  if (
    header !== expectedHeader ||
    content !== `${header}\n${verified.final.code.replace(/\r\n/g, "\n").trimEnd()}\n`
  )
    throw new Error("classic generation physical header/body/tail differs");
  return Object.freeze({
    generation: verified,
    header,
    contentSha256: runtimeBundleChecksum(content),
    physicalMap: await (await import("./runtime-rollup-bundler")).classicPhysicalMap(
      verified.final,
      header,
      file,
    ),
  });
}
export async function verifyClassicGeneration(
  value: ClassicGenerationEvidence,
  content: string,
  file: string,
) {
  const evidence = classicCompletedGenerationEvidenceSchema.parse(value);
  const expected = await completeClassicGeneration(
    evidence.generation,
    evidence.header,
    content,
    file,
  );
  const { inputPins, ...payload } = evidence;
  await new (await import("./runtime-bundle-command")).RuntimeBundleCommand(
    process.cwd(),
    "check",
  ).verifyGenerationInputPins(inputPins, evidence.generation);
  if (JSON.stringify(expected) !== JSON.stringify(payload))
    throw new Error("classic generation physical evidence differs");
  return Object.freeze({ ...expected, inputPins });
}
