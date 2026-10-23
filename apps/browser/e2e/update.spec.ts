// RC N2 R14：alterd 检查到新版本时，侧边栏底部显示提示条；点它打开 Release 页，点「×」隐藏
import { expect, test } from "@playwright/test";

import { dumpLogsOnFailure, launchApp } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { contentUrls, expectAllWithin, now, ready, uiState, waitUntilAsync } from "./ui";

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

test("RC N2 R14.S1 5 秒内显示更新提示条，点它打开 Release 页，点 × 后隐藏", async () => {
  const t0 = now();
  const app = await launchApp({
    ALTER_RELEASE_API: `http://127.0.0.1:${server.port}/api`,
    ALTER_UPDATE_CHECK_DELAY_MS: "0",
  });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await expectAllWithin(t0, 5_000, "R14.S1", () =>
      win.evaluate((): true | string => {
        const el = document.querySelector<HTMLElement>('[data-testid="update-banner"]');
        if (!el) return "还没有提示条";
        const text = el.querySelector('[data-testid="update-open"]')?.textContent ?? "";
        if (text !== "新版本 9.0.0 可用 · 运行 alter upgrade") return `提示条文字为「${text}」`;
        const rect = el.getBoundingClientRect();
        if (getComputedStyle(el).display === "none" || rect.width === 0 || rect.height === 0) return "提示条看不见";
        return true;
      }),
    );

    const before = (await uiState(win)).rows.length;
    await win.getByTestId("update-open").click();
    const url = server.url("/release");
    const opened = await waitUntilAsync(3_000, async () => (await contentUrls(app)).includes(url));
    expect(opened, `内容区应当打开 ${url}，实际 ${JSON.stringify(await contentUrls(app))}`).toBe(true);
    await expect(win.getByTestId("tab-row")).toHaveCount(before + 1, { timeout: 2_000 });
    const active = (await uiState(win)).rows.filter((r) => r.active);
    expect(active.map((r) => r.text), JSON.stringify(active)).toEqual(["Release v9.0.0"]);

    await win.getByTestId("update-dismiss").click();
    await expect(win.getByTestId("update-banner")).toHaveCount(0, { timeout: 1_000 });
  } finally {
    await app.close();
  }
});
