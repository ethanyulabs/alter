// N4 Peek：Shift+点击链接在浮层里预览；Esc 关闭，「在新标签打开」转为标签
import { expect, test, type ElectronApplication, type Page } from "@playwright/test";

import { dumpLogsOnFailure, launchApp } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { contentUrls, expectAllWithin, now, openNewTab, ready, uiState } from "./ui";

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

/** 在 URL 为 `pageUrl` 的页面视图里按住 Shift 点击文字为 `text` 的链接（测试代码读坐标，产品代码不参与）。 */
async function shiftClick(app: ElectronApplication, pageUrl: string, text: string): Promise<void> {
  await app.evaluate(
    async ({ webContents }, { pageUrl, text }) => {
      const wc = webContents.getAllWebContents().find((w) => w.getURL() === pageUrl);
      if (!wc) throw new Error(`没有 ${pageUrl} 的页面`);
      const r = (await wc.executeJavaScript(
        `(() => { const a = [...document.querySelectorAll("a")].find((x) => x.textContent === ${JSON.stringify(text)});
                  const b = a.getBoundingClientRect(); return { x: b.left + b.width / 2, y: b.top + b.height / 2 }; })()`,
      )) as { x: number; y: number };
      const at = { x: Math.round(r.x), y: Math.round(r.y), modifiers: ["shift" as const] };
      wc.sendInputEvent({ type: "mouseDown", button: "left", clickCount: 1, ...at });
      wc.sendInputEvent({ type: "mouseUp", button: "left", clickCount: 1, ...at });
    },
    { pageUrl, text },
  );
}

/** 挂在窗口上、带 Peek 标记的视图：URL 与相对内容区（侧边栏右侧）的宽高比例。 */
async function peeks(app: ElectronApplication): Promise<{ url: string; width: number; height: number; areaW: number; areaH: number }[]> {
  return app.evaluate(({ BrowserWindow, WebContentsView }) => {
    const marked = (globalThis as { __alterPeekViews?: WeakSet<object> }).__alterPeekViews;
    const win = BrowserWindow.getAllWindows()[0]!;
    const [w, h] = win.getContentSize();
    return win.contentView.children
      .filter((v) => v instanceof WebContentsView && marked?.has(v))
      .map((v) => {
        const b = v.getBounds();
        return { url: (v as Electron.WebContentsView).webContents.getURL(), width: b.width, height: b.height, areaW: w! - 240, areaH: h! };
      });
  });
}

/** 打开 /links 并 Shift+点击 Open B，等到 Peek 显示 /b。 */
async function openPeek(app: ElectronApplication, win: Page): Promise<void> {
  await openNewTab(win, server.input("/links"));
  await expect(win.getByTestId("tab-row")).toHaveText(["Links"], { timeout: 3_000 });
  await shiftClick(app, server.url("/links"), "Open B");
  await expectAllWithin(now(), 3_000, "Peek 显示 /b", async () => {
    const p = await peeks(app);
    return (p.length === 1 && p[0]!.url === server.url("/b")) || JSON.stringify(p);
  });
}

test("N4 R10.S1 打开 Peek：/b 的 Peek 视图挂在窗口上，宽高为内容区的 80%，标签仍只有 1 行", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/links"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Links"], { timeout: 3_000 });

    const t0 = now();
    await shiftClick(app, server.url("/links"), "Open B");
    await expectAllWithin(t0, 2_000, "R10.S1", async () => {
      const p = await peeks(app);
      const rows = (await uiState(win)).rows.length;
      const ok =
        p.length === 1 &&
        p[0]!.url === server.url("/b") &&
        Math.abs(p[0]!.width - p[0]!.areaW * 0.8) <= 1 &&
        Math.abs(p[0]!.height - p[0]!.areaH * 0.8) <= 1 &&
        rows === 1;
      return ok || JSON.stringify({ p, rows });
    });
  } finally {
    await app.close();
  }
});

test("N4 R11.S1 Esc 关闭：1 秒内不再挂载 Peek，也没有 /b 的 webContents", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openPeek(app, win);

    const t0 = now();
    await win.keyboard.press("Escape");
    await expectAllWithin(t0, 1_000, "R11.S1", async () => {
      const p = await peeks(app);
      const urls = await contentUrls(app);
      return (p.length === 0 && !urls.includes(server.url("/b"))) || JSON.stringify({ p, urls });
    });
  } finally {
    await app.close();
  }
});

test("N4 R11.S2 转为标签：2 秒内侧边栏依次为 Links、Page B，Page B 激活，Peek 已关闭", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openPeek(app, win);

    const t0 = now();
    await win.getByTestId("peek-promote").click();
    await expectAllWithin(t0, 2_000, "R11.S2", async () => {
      const rows = (await uiState(win)).rows.map((r) => [r.text, r.active]);
      const p = await peeks(app);
      const want = [
        ["Links", false],
        ["Page B", true],
      ];
      return (JSON.stringify(rows) === JSON.stringify(want) && p.length === 0) || JSON.stringify({ rows, p });
    });
  } finally {
    await app.close();
  }
});

async function clickMenu(app: ElectronApplication, id: string): Promise<void> {
  await app.evaluate(({ Menu }, itemId) => {
    const item = Menu.getApplicationMenu()?.getMenuItemById(itemId);
    if (!item) throw new Error(`应用菜单里没有 ${itemId}`);
    item.click();
  }, id);
}

test("N4 CR1 Peek 打开时按 Cmd+T：Peek 关闭，命令栏可见且输入框聚焦，窗口上不挂载任何页面", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openPeek(app, win);

    const t0 = now();
    await clickMenu(app, "tab.new");
    await expectAllWithin(t0, 1_000, "CR1", async () => {
      const p = await peeks(app);
      const views = await app.evaluate(({ BrowserWindow, WebContentsView }) =>
        BrowserWindow.getAllWindows()[0]!.contentView.children.filter((v) => v instanceof WebContentsView).length,
      );
      const focused = await win.evaluate(() => document.activeElement?.getAttribute("data-testid") === "command-input");
      return (p.length === 0 && views === 0 && focused) || JSON.stringify({ p, views, focused });
    });
  } finally {
    await app.close();
  }
});

test("N4 CR2 Peek 打开时切到新建的 Space：Peek 关闭，没有 /b 的 webContents", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openPeek(app, win);

    await win.getByTestId("space-add").click();
    await win.getByTestId("space-name-input").fill("Work");
    const t0 = now();
    await win.getByTestId("space-save").click();
    await expectAllWithin(t0, 1_000, "CR2", async () => {
      const p = await peeks(app);
      const urls = await contentUrls(app);
      const name = await win.getByTestId("space-name").textContent();
      return (name === "Work" && p.length === 0 && !urls.includes(server.url("/b"))) || JSON.stringify({ name, p, urls });
    });
  } finally {
    await app.close();
  }
});

test("N4 V6 点击浮层外的区域关闭 Peek：1 秒内不再挂载 Peek", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openPeek(app, win);

    // 遮罩左上角在 Peek 视图之外（Peek 居中、占 80%）
    const t0 = now();
    await win.getByTestId("peek-backdrop").click({ position: { x: 5, y: 5 } });
    await expectAllWithin(t0, 1_000, "V6", async () => {
      const p = await peeks(app);
      const backdrop = await win.getByTestId("peek-backdrop").count();
      const urls = await contentUrls(app);
      return (p.length === 0 && backdrop === 0 && !urls.includes(server.url("/b"))) || JSON.stringify({ p, backdrop, urls });
    });
  } finally {
    await app.close();
  }
});
