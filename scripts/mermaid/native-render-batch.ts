import pMap from "p-map";
import type { Fixture } from "./diagram_update_fixtures";
import type { NativeRenderRow } from "./native-render-provenance";

export interface NativeRenderBatchResult {
  row: NativeRenderRow | null;
  renderError: string | null;
}

export interface NativeRenderBatchSummary {
  results: Array<{ fixture: Fixture; result: NativeRenderBatchResult }>;
  errors: string[];
}

export async function renderNativeFixtureBatch(
  fixtures: Fixture[],
  concurrency: 1 | 2,
  render: (fixture: Fixture) => Promise<NativeRenderBatchResult>,
): Promise<NativeRenderBatchSummary> {
  let stopScheduling = false;
  const mapped = await pMap(
    (async function* (): AsyncGenerator<Fixture> {
      for (const fixture of fixtures) {
        if (stopScheduling) return;
        yield fixture;
      }
    })(),
    async (fixture) => {
      try {
        const result = await render(fixture);
        if (result.renderError !== null) stopScheduling = true;
        return { fixture, result, error: null };
      } catch (error) {
        stopScheduling = true;
        const detail = error instanceof Error ? error.message : String(error);
        return {
          fixture,
          result: { row: null, renderError: detail },
          error: `${fixture.slug}: ${detail}`,
        };
      }
    },
    { concurrency, stopOnError: false },
  );
  return {
    results: mapped.map(({ fixture, result }) => ({ fixture, result })),
    errors: mapped.flatMap(({ error }) => (error === null ? [] : [error])),
  };
}
