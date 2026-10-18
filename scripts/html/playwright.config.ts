import { defineConfig } from "playwright/test";

// WHY: Playwright Test の外部APIが設定のdefault exportを要求する。
export default defineConfig({
  testMatch: ["feature-catalog_test.ts", "chrome_badge_metrics_test.ts"],
  timeout: 5000,
  retries: 0,
  workers: 1,
  fullyParallel: false,
  forbidOnly: true,
  outputDir: "../../tmp/quality/browser-contract/artifacts",
  use: {
    channel: "chrome",
    headless: true,
    viewport: { width: 1280, height: 900 },
  },
});
