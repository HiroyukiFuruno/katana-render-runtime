import fs from "node:fs";
import path from "node:path";
import { RasterComparisonBinding } from "./raster-comparison-binding";
import type { RasterCaptureState } from "./raster-production-binding";
import { rasterSha256 } from "./raster-production-binding";

export function rasterCaptureFixture(svg: Buffer, domHashSource: string): RasterCaptureState {
  const canonicalDomHash = rasterSha256(Buffer.from("synthetic canonical SVG DOM"));
  const domHash = rasterSha256(Buffer.from(domHashSource));
  return {
    domSha256Before: domHash,
    domSha256After: domHash,
    svg: {
      policy: "browser-domparser-html-svg-outerhtml-v1",
      inputSha256: rasterSha256(svg),
      expectedDomSha256: canonicalDomHash,
      assignedDomSha256: canonicalDomHash,
      beforeDomSha256: canonicalDomHash,
      endDomSha256: canonicalDomHash,
    },
    bounds: { x: 0, y: 0, width: 1, height: 1 },
    viewport: { width: 1, height: 1 },
    deviceScaleFactor: 1,
    theme: "dark",
    resized: false,
    fontsReady: true,
    imagesDecoded: true,
  };
}

type ComparisonFixturePaths = {
  directory: string;
  configPath: string;
  originalExpectedSha: string;
};

export function prepareNestedRuntimeAsset(fixture: ComparisonFixturePaths) {
  const nested = path.join(fixture.directory, "scripts");
  fs.mkdirSync(nested);
  fs.writeFileSync(path.join(nested, "runtime.js"), "same bytes");
  const binding = RasterComparisonBinding.read(fixture.configPath, fixture.originalExpectedSha, [
    "unit",
  ]);
  return { binding, nested };
}
