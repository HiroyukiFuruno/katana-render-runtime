import { defineConfig } from "playwright/test";
import { browserContractDefaults } from "./playwright-config-base";

// WHY: macOSの実プロセス観測用。Playwrightの外部APIが設定のdefault exportを要求する。
export default defineConfig({
  testDir: "..",
  testMatch: ["mermaid/official_raw_producer_macos_browser_test.ts"],
  ...browserContractDefaults,
  outputDir: "../../tmp/quality/macos-browser-contract/artifacts",
});
