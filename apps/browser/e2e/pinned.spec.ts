// N1 tabs 能力：固定分区、图钉按钮、关闭固定标签只卸载页面、拖拽排序
import { expect, test, type ElectronApplication, type Page } from "@playwright/test";

import { dumpLogsOnFailure, launchApp, tempHome } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import {
  attachedUrls,
  contentUrls,
  expectAllWithin,
  now,
  openNewTab,
  queryDb,
  ready,
  uiState,
  waitUntilAsync,
} from "./ui";

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

function row(win: Page, title: string) {
  return win.getByTestId("tab-row").filter({ hasText: title });
}

/** 依次新开标签并等到侧边栏按顺序显示这些标题。 */
async function openAll(win: Page, paths: string[], titles: string[]): Promise<void> {
  for (const path of paths) await openNewTab(win, server.input(path));
  await expect(win.getByTestId("tab-row")).toHaveText(titles, { timeout: 3_000 });
}

/** 鼠标移到行上，点它的图钉按钮。 */
async function togglePin(win: Page, title: string): Promise<void> {
  await row(win, title).hover();
  await row(win, title).getByTestId("tab-pin").click();
}

/** 固定分区（分隔线上方）里各行的标题。 */
async function pinnedTitles(win: Page): Promise<string[]> {
  return win.getByTestId("tab-row").and(win.locator('[data-pinned="true"]')).allTextContents();
}

/** 正常退出后以同一个 ALTER_HOME 重启；`t0` 是重新启动前的时刻。 */
async function restart(
  home: string,
  before: ElectronApplication,
): Promise<{ app: ElectronApplication; win: Page; t0: number }> {
  await before.close();
  const t0 = now();
  const app = await launchApp({ ALTER_HOME: home });
  const win = await app.firstWindow();
  return { app, win, t0 };
}

test("N1 R8.S4 在侧边栏固定并在重启后保持：Page B 排到最前且在固定分区", async () => {
  const home = tempHome();
  let app = await launchApp({ ALTER_HOME: home });
  try {
    let win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a", "/b"], ["Page A", "Page B"]);

    await togglePin(win, "Page B");
    await expect(win.getByTestId("tab-row")).toHaveText(["Page B", "Page A"], { timeout: 1_000 });

    let t0: number;
    ({ app, win, t0 } = await restart(home, app));
    // 顺序为 B、A，B 在分隔线上方且固定，A 在下方
    await expectAllWithin(t0, 3_000, "R8.S4", async () => {
      const s = await uiState(win);
      const [b, a] = s.rows;
      const ok =
        s.rows.length === 2 &&
        b!.text === "Page B" &&
        b!.pinned &&
        b!.aboveDivider &&
        a!.text === "Page A" &&
        !a!.pinned &&
        !a!.aboveDivider;
      return ok || JSON.stringify(s.rows);
    });
  } finally {
    await app.close();
  }
});

test("N1 R10.S2 在侧边栏关闭固定标签后再打开：行仍在固定分区，/b 被卸载，点它回到 /a", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a", "/c"], ["Page A", "Page C"]);
    await togglePin(win, "Page A");
    await row(win, "Page A").click();
    await expect(row(win, "Page A")).toHaveClass(/active/);
    // 在固定标签里导航到 /b
    await win.getByTestId("address-input").fill(server.input("/b"));
    await win.getByTestId("address-input").press("Enter");
    await expect(win.getByTestId("tab-row")).toHaveText(["Page B", "Page C"], { timeout: 2_000 });

    // 被关闭的就是固定的这一行：记下它在侧边栏上的 id
    const pinnedId = (await uiState(win)).rows.find((r) => r.pinned)!.id;
    await row(win, "Page B").hover();
    let t0 = now();
    await row(win, "Page B").getByTestId("tab-close").click();

    // 同一次界面读取里：保留在固定分区（分隔线上方）的正是被关闭的那一行，并且它的页面已卸载
    await expectAllWithin(t0, 1_000, "R10.S2 关闭", async () => {
      const s = await uiState(win);
      const urls = await contentUrls(app);
      const kept = s.rows[0];
      const ok =
        s.rows.length === 2 &&
        kept?.id === pinnedId &&
        kept.pinned &&
        kept.aboveDivider &&
        !urls.includes(server.url("/b"));
      return ok || JSON.stringify({ rows: s.rows, urls });
    });

    t0 = now();
    await win.getByTestId("tab-row").first().click();
    await expectAllWithin(t0, 2_000, "R10.S2 再打开", async () => {
      const attached = await attachedUrls(app);
      return attached.join() === server.url("/a") || JSON.stringify(attached);
    });
  } finally {
    await app.close();
  }
});

test("N1 R11.S5 在侧边栏拖动并在重启后保持：Page C 拖到 Page A 上方后依次为 C、A、B", async () => {
  const home = tempHome();
  let app = await launchApp({ ALTER_HOME: home });
  try {
    let win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a", "/b", "/c"], ["Page A", "Page B", "Page C"]);

    // 落在 Page A 那一行的上四分之一处，即「Page A 上方」
    const target = (await row(win, "Page A").boundingBox())!;
    await row(win, "Page C").dragTo(row(win, "Page A"), { targetPosition: { x: 20, y: target.height / 4 } });
    await expect(win.getByTestId("tab-row")).toHaveText(["Page C", "Page A", "Page B"], { timeout: 1_000 });

    ({ app, win } = await restart(home, app));
    await expect(win.getByTestId("tab-row")).toHaveText(["Page C", "Page A", "Page B"], { timeout: 3_000 });
  } finally {
    await app.close();
  }
});

test("N1 CR1 拖到分隔线即固定：Page B 拖到分隔线后排到最前并在固定分区", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a", "/b"], ["Page A", "Page B"]);

    await row(win, "Page B").dragTo(win.getByTestId("pinned-divider"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Page B", "Page A"], { timeout: 1_000 });
    expect(await pinnedTitles(win)).toEqual(["Page B"]);
  } finally {
    await app.close();
  }
});

test("N1 CR2 全部固定后拖到普通分区空白处即取消固定", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a"], ["Page A"]);
    await togglePin(win, "Page A");
    await expect(row(win, "Page A")).toHaveAttribute("data-pinned", "true", { timeout: 1_000 });

    await row(win, "Page A").dragTo(win.getByTestId("normal-drop"));
    await expect(row(win, "Page A")).toHaveAttribute("data-pinned", "false", { timeout: 1_000 });
  } finally {
    await app.close();
  }
});

test("N1 CR4 只有一个固定标签时关闭它：页面保持卸载，点击该行后才重新打开", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a"], ["Page A"]);
    await togglePin(win, "Page A");
    await expect(row(win, "Page A")).toHaveAttribute("data-pinned", "true", { timeout: 1_000 });

    await row(win, "Page A").hover();
    await row(win, "Page A").getByTestId("tab-close").click();
    // 消息顺序不固定：1 秒后再看，页面也不应被快照重新建出来
    await win.waitForTimeout(1_000);
    expect(await contentUrls(app)).not.toContain(server.url("/a"));
    await expect(win.getByTestId("tab-row")).toHaveCount(1);

    await row(win, "Page A").click();
    expect(await waitUntilAsync(2_000, async () => (await attachedUrls(app)).join() === server.url("/a"))).toBe(true);
  } finally {
    await app.close();
  }
});

test("N1 CR10 关闭固定标签后在 discard 到达前再次激活：页面按固定地址 /a 打开", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a"], ["Page A"]);
    await togglePin(win, "Page A");
    await expect(row(win, "Page A")).toHaveAttribute("data-pinned", "true", { timeout: 1_000 });
    await win.getByTestId("address-input").fill(server.input("/b"));
    await win.getByTestId("address-input").press("Enter");
    await expect(win.getByTestId("tab-row")).toHaveText(["Page B"], { timeout: 2_000 });
    const tabId = await app.evaluate(async ({ BrowserWindow }) => {
      const wc = BrowserWindow.getAllWindows()[0]!.webContents;
      return (await wc.executeJavaScript(
        `window.alter.getShellView().then((v) => v.state.tabs[0].id)`,
      )) as string;
    });

    // 同一轮事件循环里连发：Main 依次收到关闭与激活，二者都早于 alterd 的 shell.discard
    await win.evaluate((id) => {
      const alter = (window as unknown as { alter: { closeTab(id: string): void; activateTab(id: string): void } }).alter;
      alter.closeTab(id);
      alter.activateTab(id);
    }, tabId);

    expect(
      await waitUntilAsync(2_000, async () => (await attachedUrls(app)).join() === server.url("/a")),
      `内容区应当显示 /a，实际 ${JSON.stringify(await attachedUrls(app))}`,
    ).toBe(true);
  } finally {
    await app.close();
  }
});

test("N1 CR27 关闭唯一的固定标签：内容区显示没有打开的标签，该行不是激活样式，点击后重新打开 /a", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a"], ["Page A"]);
    await togglePin(win, "Page A");
    await expect(row(win, "Page A")).toHaveAttribute("data-pinned", "true", { timeout: 1_000 });
    await row(win, "Page A").hover();
    await row(win, "Page A").getByTestId("tab-close").click();

    await expect(win.getByTestId("empty-content")).toHaveText("没有打开的标签", { timeout: 1_000 });
    await expect(row(win, "Page A")).not.toHaveClass(/active/);

    await row(win, "Page A").click();
    expect(await waitUntilAsync(2_000, async () => (await attachedUrls(app)).join() === server.url("/a"))).toBe(true);
    await expect(row(win, "Page A")).toHaveClass(/active/);
  } finally {
    await app.close();
  }
});

test("N1 CR24 唯一的固定标签关闭后在地址框打开 /b：新开一个标签显示 /b，固定标签留在固定分区", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a"], ["Page A"]);
    await togglePin(win, "Page A");
    await expect(row(win, "Page A")).toHaveAttribute("data-pinned", "true", { timeout: 1_000 });
    await row(win, "Page A").hover();
    await row(win, "Page A").getByTestId("tab-close").click();
    expect(await waitUntilAsync(1_000, async () => (await attachedUrls(app)).length === 0)).toBe(true);

    await win.getByTestId("address-input").fill(server.input("/b"));
    await win.getByTestId("address-input").press("Enter");
    expect(
      await waitUntilAsync(2_000, async () => (await attachedUrls(app)).join() === server.url("/b")),
      `内容区应当显示 /b，实际 ${JSON.stringify(await attachedUrls(app))}`,
    ).toBe(true);
    await expect(win.getByTestId("tab-row")).toHaveText(["Page A", "Page B"], { timeout: 2_000 });
    expect(await pinnedTitles(win)).toEqual(["Page A"]);
  } finally {
    await app.close();
  }
});

test("N1 CR29 关闭唯一的固定标签后立即在地址框打开 /b：新开标签显示 /b，固定标签不被改写", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a"], ["Page A"]);
    await togglePin(win, "Page A");
    await expect(row(win, "Page A")).toHaveAttribute("data-pinned", "true", { timeout: 1_000 });

    // 同一轮事件循环里连发：关闭的快照到达之前就提交地址（不是新标签模式）
    await win.evaluate(
      async ({ input }) => {
        const alter = (window as unknown as {
          alter: {
            getShellView(): Promise<{ state: { tabs: { id: string }[] } }>;
            closeTab(id: string): void;
            open(r: { input: string; newTab: boolean }): void;
          };
        }).alter;
        const id = (await alter.getShellView()).state.tabs[0]!.id;
        alter.closeTab(id);
        alter.open({ input, newTab: false });
      },
      { input: server.input("/b") },
    );

    await expect(win.getByTestId("tab-row")).toHaveCount(2, { timeout: 2_000 });
    expect(await waitUntilAsync(2_000, async () => (await attachedUrls(app)).join() === server.url("/b"))).toBe(true);
    expect(queryDb(home, "SELECT url FROM tabs WHERE pinned = 1")).toEqual([{ url: server.url("/a") }]);
  } finally {
    await app.close();
  }
});
