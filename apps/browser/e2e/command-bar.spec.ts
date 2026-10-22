// N4 命令栏：Cmd+T（菜单项 tab.new）打开，搜索标签、Space、历史，回车执行，Esc 关闭
import { expect, test, type ElectronApplication, type Page } from "@playwright/test";

import { dumpLogsOnFailure, launchApp } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { attachedUrls, expectAllWithin, now, openNewTab, ready, uiState } from "./ui";

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

async function clickMenu(app: ElectronApplication, id: string): Promise<void> {
  await app.evaluate(({ Menu }, itemId) => {
    const item = Menu.getApplicationMenu()?.getMenuItemById(itemId);
    if (!item) throw new Error(`应用菜单里没有 ${itemId}`);
    item.click();
  }, id);
}

async function openAll(win: Page, paths: string[], titles: string[]): Promise<void> {
  for (const path of paths) await openNewTab(win, server.input(path));
  await expect(win.getByTestId("tab-row")).toHaveText(titles, { timeout: 3_000 });
}

/** 打开命令栏并键入 `text`，等到结果里出现 `kind` 的项。 */
async function typeInBar(app: ElectronApplication, win: Page, text: string, kind: string): Promise<void> {
  await clickMenu(app, "tab.new");
  await expect(win.getByTestId("command-input")).toBeFocused({ timeout: 1_000 });
  await win.keyboard.type(text);
  await expect(win.locator(`[data-testid="command-item"][data-kind="${kind}"]`).first()).toBeVisible({
    timeout: 2_000,
  });
}

/** 各行的 [标题, 是否激活]。 */
async function rows(win: Page): Promise<[string, boolean][]> {
  return (await uiState(win)).rows.map((r) => [r.text, r.active]);
}

test("N4 R2.S1 打开与关闭：tab.new 后命令栏可见、输入框聚焦、不挂载页面；Esc 后恢复显示 /a", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a"], ["Page A"]);

    const t0 = now();
    await clickMenu(app, "tab.new");
    await expectAllWithin(t0, 1_000, "R2.S1 打开", async () => {
      const visible = await win.getByTestId("command-bar").isVisible();
      const focused = await win.evaluate(
        () => document.activeElement?.getAttribute("data-testid") === "command-input",
      );
      const attached = await attachedUrls(app);
      return (visible && focused && attached.length === 0) || JSON.stringify({ visible, focused, attached });
    });

    const t1 = now();
    await win.keyboard.press("Escape");
    await expectAllWithin(t1, 1_000, "R2.S1 关闭", async () => {
      const visible = await win.getByTestId("command-bar").isVisible();
      const attached = await attachedUrls(app);
      return (!visible && JSON.stringify(attached) === JSON.stringify([server.url("/a")])) ||
        JSON.stringify({ visible, attached });
    });
  } finally {
    await app.close();
  }
});

test("N4 R4.S1 切到另一个 Space 的标签：回车后底部为 Personal、Page A 激活、命令栏关闭", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a"], ["Page A"]);
    await win.getByTestId("space-add").click();
    await win.getByTestId("space-name-input").fill("Work");
    await win.getByTestId("space-save").click();
    await expect(win.getByTestId("space-name")).toHaveText("Work", { timeout: 2_000 });

    await typeInBar(app, win, "Page A", "tab");
    const t0 = now();
    await win.keyboard.press("Enter");
    await expectAllWithin(t0, 1_000, "R4.S1", async () => {
      const space = await win.getByTestId("space-name").textContent();
      const r = await rows(win);
      const bar = await win.getByTestId("command-bar").isVisible();
      return (space === "Personal" && JSON.stringify(r) === JSON.stringify([["Page A", true]]) && !bar) ||
        JSON.stringify({ space, r, bar });
    });
  } finally {
    await app.close();
  }
});

test("N4 R4.S2 输入网址打开新标签：2 秒内依次为 Page A、Page B，Page B 激活", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a"], ["Page A"]);

    await typeInBar(app, win, server.input("/b"), "open");
    const t0 = now();
    await win.keyboard.press("Enter");
    await expectAllWithin(t0, 2_000, "R4.S2", async () => {
      const r = await rows(win);
      const want = [
        ["Page A", false],
        ["Page B", true],
      ];
      return JSON.stringify(r) === JSON.stringify(want) || JSON.stringify(r);
    });
  } finally {
    await app.close();
  }
});

test("N4 R4.S3 打开历史项：方向键选中历史项回车，2 秒内出现 Page C 且激活", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a", "/c"], ["Page A", "Page C"]);
    await win.getByTestId("tab-row").filter({ hasText: "Page C" }).hover();
    await win.getByTestId("tab-row").filter({ hasText: "Page C" }).getByTestId("tab-close").click();
    await expect(win.getByTestId("tab-row")).toHaveText(["Page A"], { timeout: 2_000 });

    await typeInBar(app, win, "Page C", "history");
    // 方向键移到历史项
    for (let i = 0; i < 6; i++) {
      const kind = await win.locator('[data-testid="command-item"][aria-selected="true"]').getAttribute("data-kind");
      if (kind === "history") break;
      await win.keyboard.press("ArrowDown");
    }
    await expect(win.locator('[data-testid="command-item"][aria-selected="true"]')).toHaveAttribute(
      "data-kind",
      "history",
    );
    const t0 = now();
    await win.keyboard.press("Enter");
    await expectAllWithin(t0, 2_000, "R4.S3", async () => {
      const r = await rows(win);
      return r.some(([t, a]) => t === "Page C" && a) || JSON.stringify(r);
    });
  } finally {
    await app.close();
  }
});

test("N4 V4 执行 Space 结果项：当前为 Work 时输入 Personal 回车，1 秒内切到 Personal", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await win.getByTestId("space-add").click();
    await win.getByTestId("space-name-input").fill("Work");
    await win.getByTestId("space-save").click();
    await expect(win.getByTestId("space-name")).toHaveText("Work", { timeout: 2_000 });

    await typeInBar(app, win, "Personal", "space");
    await expect(win.locator('[data-testid="command-item"][aria-selected="true"]')).toHaveAttribute("data-kind", "space");
    const t0 = now();
    await win.keyboard.press("Enter");
    await expectAllWithin(t0, 1_000, "V4", async () => {
      const name = await win.getByTestId("space-name").textContent();
      const bar = await win.getByTestId("command-bar").isVisible();
      return (name === "Personal" && !bar) || JSON.stringify({ name, bar });
    });
  } finally {
    await app.close();
  }
});
