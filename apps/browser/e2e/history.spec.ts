// N4 浏览历史：iframe 里的页内导航不写历史
import { expect, test } from "@playwright/test";

import { dumpLogsOnFailure, launchApp, tempHome } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { openNewTab, queryDb, ready, waitUntilAsync } from "./ui";

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

test("N4 R1.S2 iframe 导航不写历史：2 秒后 history 中只有 /frame 的 1 条记录", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/frame"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Frame"], { timeout: 3_000 });
    // 先确认 iframe 确实改写了自己的 hash（iframe 里的页内导航已经发生），再从这一刻起等 2 秒
    const frameUrl = server.url("/frame");
    const hashed = await waitUntilAsync(3_000, async () =>
      app.evaluate(async ({ webContents }, url) => {
        const wc = webContents.getAllWebContents().find((w) => w.getURL() === url);
        if (!wc) return false;
        return (await wc.executeJavaScript(
          `document.querySelector("iframe")?.contentWindow?.location.hash === "#x"`,
        )) as boolean;
      }, frameUrl),
    );
    expect(hashed, "iframe 没有改写 hash").toBe(true);
    await win.waitForTimeout(2_000);
    const rows = queryDb(home, "SELECT url FROM history ORDER BY visited_at").map((r) => r["url"]);
    expect(rows).toEqual([server.url("/frame")]);
  } finally {
    await app.close();
  }
});
