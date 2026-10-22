// N4 分屏：侧边栏右键「与当前标签分屏」，内容区左右各半；重启后保持
import { expect, test, type ElectronApplication, type Page } from "@playwright/test";

import { dumpLogsOnFailure, launchApp, tempHome } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { expectAllWithin, now, openNewTab, ready } from "./ui";

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

/** 窗口上挂载的页面视图，按 x 从左到右：URL 与位置、大小。 */
async function mounted(
  app: ElectronApplication,
): Promise<{ url: string; x: number; y: number; width: number; height: number }[]> {
  const views = await app.evaluate(({ BrowserWindow, WebContentsView }) =>
    BrowserWindow.getAllWindows()
      .flatMap((w) => w.contentView.children)
      .filter((v) => v instanceof WebContentsView)
      .map((v) => ({ url: v.webContents.getURL(), ...v.getBounds() })),
  );
  return views.sort((a, b) => a.x - b.x);
}

/** 内容区（侧边栏右侧）的宽度。 */
async function contentWidth(app: ElectronApplication): Promise<number> {
  return app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.getContentSize()[0]! - 240);
}

/** 内容区的高度。 */
async function contentHeight(app: ElectronApplication): Promise<number> {
  return app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.getContentSize()[1]!);
}

/** 左边 /b、右边 /a，宽度之差不超过 1 px，两者合起来正好铺满内容区、各占一半。 */
async function sideBySide(app: ElectronApplication): Promise<true | string> {
  const v = await mounted(app);
  const area = await contentWidth(app);
  const height = await contentHeight(app);
  const ok =
    v.length === 2 &&
    v.every((x) => x.y === 0 && x.height === height) &&
    v[0]!.url === server.url("/b") &&
    v[1]!.url === server.url("/a") &&
    Math.abs(v[0]!.width - v[1]!.width) <= 1 &&
    v[0]!.x === 240 &&
    v[1]!.x === 240 + v[0]!.width &&
    v[0]!.width + v[1]!.width === area &&
    Math.abs(v[0]!.width - area / 2) <= 1;
  return ok || JSON.stringify({ v, area, height });
}

async function openAll(win: Page, paths: string[], titles: string[]): Promise<void> {
  for (const path of paths) await openNewTab(win, server.input(path));
  await expect(win.getByTestId("tab-row")).toHaveText(titles, { timeout: 3_000 });
}

test("N4 R6.S4 在侧边栏分屏：左 /b 右 /a、宽度相差不超过 1 px，重启后仍并排", async () => {
  const home = tempHome();
  let app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a", "/b"], ["Page A", "Page B"]);

    await win.getByTestId("tab-row").filter({ hasText: "Page A" }).click({ button: "right" });
    const t0 = now();
    await win.getByTestId("tab-menu-split").click();
    await expectAllWithin(t0, 1_000, "R6.S4 分屏", () => sideBySide(app));
  } finally {
    await app.close();
  }
  // 正常退出后以同一个 ALTER_HOME 重启
  app = await launchApp({ ALTER_HOME: home });
  try {
    await ready(await app.firstWindow());
    await expectAllWithin(now(), 5_000, "R6.S4 重启后", () => sideBySide(app));
  } finally {
    await app.close();
  }
});

test("N4 V5 激活分屏中的另一个仍并排、左右不变；取消分屏后只显示激活的那个", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a", "/b"], ["Page A", "Page B"]);
    const rowA = win.getByTestId("tab-row").filter({ hasText: "Page A" });
    await rowA.click({ button: "right" });
    const tSplit = now();
    await win.getByTestId("tab-menu-split").click();
    await expectAllWithin(tSplit, 2_000, "分屏", () => sideBySide(app));

    // 激活右边的 Page A：仍是左 /b 右 /a
    const t0 = now();
    await rowA.click();
    await expectAllWithin(t0, 1_000, "激活另一个", async () => {
      const active = await rowA.getAttribute("class");
      const s = await sideBySide(app);
      return (s === true && (active ?? "").includes("active")) || JSON.stringify({ s, active });
    });

    // 取消分屏：只挂激活的 Page A，占满内容区
    await rowA.click({ button: "right" });
    const t1 = now();
    await win.getByTestId("tab-menu-unsplit").click();
    await expectAllWithin(t1, 1_000, "取消分屏", async () => {
      const v = await mounted(app);
      const area = await contentWidth(app);
      const height = await contentHeight(app);
      const ok =
        v.length === 1 &&
        v[0]!.url === server.url("/a") &&
        v[0]!.x === 240 &&
        v[0]!.y === 0 &&
        v[0]!.width === area &&
        v[0]!.height === height;
      return ok || JSON.stringify({ v, area, height });
    });
  } finally {
    await app.close();
  }
});
