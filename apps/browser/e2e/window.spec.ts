// app-shell 能力：主窗口启动
import { expect, test } from "@playwright/test";

import { launchApp } from "./app";

test("R6.S1 窗口打开：标题为 alter，页面可见文本含 alter", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow({ timeout: 10_000 });
    expect(await win.title()).toBe("alter");
    await expect(win.getByText("alter", { exact: true })).toBeVisible();
  } finally {
    await app.close();
  }
});
