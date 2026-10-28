import { defineConfig } from "playwright/test";
import { browserContractDefaults } from "./playwright-config-base";

// WHY: macOSの実プロセス識別情報を観測するproducer用の設定。
export default defineConfig({
  testDir: "..",
  testMatch: ["mermaid/official_raw_producer_macos_browser_test.ts"],
  ...browserContractDefaults,
  outputDir: "../../tmp/quality/macos-browser-contract/artifacts",
});
