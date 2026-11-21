import type { Page } from "playwright";

export async function applyEventsInteraction(page: Page): Promise<void> {
  await page.locator("#field").click();
  await page.locator("#field").fill("入力");
  await page.locator("#field").press("Shift");
  await page.locator("button").click();
}
