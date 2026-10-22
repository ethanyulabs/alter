// N5 导入向导：从侧边栏菜单打开，选来源、profile、目标 Space 与类别，显示进度与计数
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { dumpLogsOnFailure, launchApp, tempHome } from "./app";
import { standardProfile } from "./fixtures/profile";
import { expectAllWithin, now, queryDb, ready, spaceIdByName } from "./ui";

// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
});

test("N5 R10.S1 从向导导入：5 秒内显示 书签 3、历史 2 与「完成」，Work 有 3 行书签", async () => {
  const home = tempHome();
  const chrome = mkdtempSync(join(tmpdir(), "alter-chrome-"));
  standardProfile(join(chrome, "Default"));
  // Arc 的数据根目录指向不存在的位置：开发机上装了 Arc 时也不受影响
  const app = await launchApp({ ALTER_HOME: home, ALTER_CHROME_DIR: chrome, ALTER_ARC_DIR: join(home, "no-arc") });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await win.getByTestId("space-add").click();
    await win.getByTestId("space-name-input").fill("Work");
    await win.getByTestId("space-save").click();
    await expect(win.getByTestId("space-name")).toHaveText("Work", { timeout: 2_000 });
    // 回到 Personal：向导默认选当前 Space，目标 Space 必须由用户选成 Work
    await win.getByTestId("space-dot").nth(0).click();
    await expect(win.getByTestId("space-name")).toHaveText("Personal", { timeout: 2_000 });

    await win.getByTestId("sidebar-menu").click();
    await win.getByTestId("menu-import").click();
    await win.getByTestId("import-source").selectOption("chrome");
    const profile = win.getByTestId("import-profile");
    await expect(profile.locator("option")).toHaveText(["Default"], { timeout: 2_000 });
    await profile.selectOption({ label: "Default" });
    await win.getByTestId("import-space").selectOption({ label: "Work" });
    const item = (value: string) => win.locator(`[data-testid="import-item"][value="${value}"]`);
    await item("bookmarks").check();
    await item("history").check();
    await expect(item("cookies")).not.toBeChecked();

    // 5 秒从点「开始导入」之前算起；三个条件要在同一次检查里同时成立
    const t0 = now();
    await win.getByTestId("import-start").click();
    // 一次同步读取同一份 DOM：分两次异步读可能读到切换前后的两个状态
    await expectAllWithin(t0, 5_000, "R10.S1", () =>
      win.evaluate((): true | string => {
        const text = document.querySelector('[data-testid="import-result"]')?.textContent ?? "";
        if (!text.includes("书签 3")) return `结果区没有「书签 3」：${text}`;
        if (!text.includes("历史 2")) return `结果区没有「历史 2」：${text}`;
        const done = document.querySelector<HTMLElement>('[data-testid="import-done"]');
        if (!done) return "还没有「完成」按钮";
        const style = getComputedStyle(done);
        const rect = done.getBoundingClientRect();
        if (style.display === "none" || style.visibility !== "visible" || Number(style.opacity) === 0 || rect.width === 0 || rect.height === 0)
          return "「完成」按钮还看不见";
        return true;
      }),
    );

    const work = spaceIdByName(home, "Work");
    expect(queryDb(home, "SELECT COUNT(*) AS n FROM bookmarks WHERE space_id = ?", work)[0]!.n).toBe(3);
  } finally {
    await app.close();
  }
});
