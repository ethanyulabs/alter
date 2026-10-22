// N4 已归档列表：侧边栏「已归档」里点一项恢复
import { expect, test } from "@playwright/test";

import { dumpLogsOnFailure, launchApp } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { expectAllWithin, now, openNewTab, ready, uiState } from "./ui";

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

test("N4 R9.S4 在侧边栏恢复：点「已归档」里的 Page A，1 秒内依次为 Page B、Page A 且 Page A 激活", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    for (const path of ["/a", "/b"]) await openNewTab(win, server.input(path));
    await expect(win.getByTestId("tab-row")).toHaveText(["Page A", "Page B"], { timeout: 3_000 });
    const rowA = win.getByTestId("tab-row").filter({ hasText: "Page A" });
    await rowA.hover();
    await rowA.getByTestId("tab-close").click();
    await expect(win.getByTestId("tab-row")).toHaveText(["Page B"], { timeout: 2_000 });

    await win.getByTestId("archived-toggle").click();
    const item = win.getByTestId("archived-item").filter({ hasText: "Page A" });
    await expect(item).toBeVisible({ timeout: 1_000 });
    const t0 = now();
    await item.click();
    await expectAllWithin(t0, 1_000, "R9.S4", async () => {
      const rows = (await uiState(win)).rows.map((r) => [r.text, r.active]);
      const want = [
        ["Page B", false],
        ["Page A", true],
      ];
      return JSON.stringify(rows) === JSON.stringify(want) || JSON.stringify(rows);
    });
  } finally {
    await app.close();
  }
});
