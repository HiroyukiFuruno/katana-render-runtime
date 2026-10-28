export const browserContractDefaults = {
  timeout: 5000,
  retries: 0,
  workers: 1,
  fullyParallel: false,
  forbidOnly: true,
  use: {
    channel: "chrome",
    headless: true,
    viewport: { width: 1280, height: 900 },
  },
} as const;
