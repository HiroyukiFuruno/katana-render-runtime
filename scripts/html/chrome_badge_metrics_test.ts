import path from "node:path";
import { expect, test } from "playwright/test";

test.describe("Chrome badge intrinsic metrics", () => {
  test("records independent text range and box widths", async ({ page }) => {
    await page.goto(
      `file://${path.resolve("tests/fixtures/html/library-reservation-fidelity.html")}`,
      { waitUntil: "networkidle" },
    );
    const metrics = await page.evaluate(() =>
      Array.from(document.querySelectorAll<HTMLElement>(".badge")).map((element) => {
        const box = element.getBoundingClientRect();
        const range = document.createRange();
        range.selectNodeContents(element);
        const style = getComputedStyle(element);
        return {
          text: element.textContent,
          boxWidth: box.width,
          textWidth: range.getBoundingClientRect().width,
          horizontalInset:
            Number.parseFloat(style.paddingLeft) + Number.parseFloat(style.paddingRight),
        };
      }),
    );
    expect(metrics).toHaveLength(2);
    expect(metrics[0]?.text).toBe("要件仕様 v1");
    expect(metrics[1]?.text).toBe("レビュー中");
    for (const metric of metrics) {
      expect(metric.textWidth).toBeGreaterThan(0);
      expect(metric.boxWidth).toBeCloseTo(metric.textWidth + metric.horizontalInset, 3);
    }
  });
});
