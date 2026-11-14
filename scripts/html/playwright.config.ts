import { defineConfig } from "playwright/test";
import { browserContractDefaults } from "./playwright-config-base";

// WHY: Playwright Test の外部APIが設定のdefault exportを要求する。
export default defineConfig({
  testDir: "..",
  testMatch: [
    "html/feature-catalog_test.ts",
    "html/chrome_badge_metrics_test.ts",
    "html/reference-capture_browser_test.ts",
    "mermaid/reference_svg_geometry_browser_test.ts",
  ],
  ...browserContractDefaults,
  outputDir: "../../tmp/quality/browser-contract/artifacts",
});
