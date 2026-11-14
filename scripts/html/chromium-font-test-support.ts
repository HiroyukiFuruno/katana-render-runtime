export interface JoinedStaticFontTraceFixture {
  events: Record<string, unknown>[];
  sessionId: string;
}

export function buildJoinedStaticFontTrace(input: {
  sessionId: string;
  processId: number;
  physicalFilePath: string;
  physicalFileSha256: string;
  descriptorSha256: string;
  descriptorLength: number;
  postScriptName?: string;
}): JoinedStaticFontTraceFixture {
  const { sessionId, processId } = input;
  const event = (name: string, args: Record<string, unknown>): Record<string, unknown> => ({
    cat: "katana.font-observer",
    name,
    pid: processId,
    args: {
      observerSessionId: sessionId,
      processId,
      processRole: "renderer",
      ...args,
    },
  });
  return {
    sessionId,
    events: [
      event("retained-paint", {
        paintSerial: 1,
        paintPhase: "begin",
        transformRowMajor: JSON.stringify(
          Array(16)
            .fill(0)
            .map((_, index) =>
              index === 0 || index === 5 || index === 10 || index === 15 ? 1 : 0,
            ),
        ),
      }),
      event("strike-cache", {
        paintSerial: 1,
        cacheHit: false,
        descriptorSHA256: input.descriptorSha256,
        descriptorLength: input.descriptorLength,
        descriptorChecksum: 1,
      }),
      event("scaler-font", {
        paintSerial: 0,
        descriptorSHA256: input.descriptorSha256,
        descriptorLength: input.descriptorLength,
        actualFont: JSON.stringify({
          postScriptName: input.postScriptName ?? "Fixture-Regular",
          physicalFileSHA256: input.physicalFileSha256,
          physicalFileAvailable: true,
          physicalFaceBindingStatus: "observed:unique-table-match",
          physicalFaceTtcIndex: 0,
          physicalFaceFvarTablePresent: false,
          axes: [],
          descriptorURL: input.physicalFilePath,
        }),
        skiaVariationPosition: JSON.stringify({ available: false, coordinates: [] }),
      }),
      event("retained-paint", { paintSerial: 1, paintPhase: "end" }),
    ],
  };
}
