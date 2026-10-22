// N1 spaces 能力：侧边栏底部的 Space 圆点、编辑面板、切换、删除与 Cookie 隔离
import { expect, test, type ElectronApplication, type Page } from "@playwright/test";

import { dumpLogsOnFailure, launchApp, tempHome } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import {
  attachedText,
  attachedUrls,
  expectAllWithin,
  now,
  openNewTab,
  queryDb,
  ready,
  spaceIdByName,
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

/** 点「+」，输入名称、选第 `colorIndex` 个颜色（从 0 开始），点「创建」。 */
async function createSpace(win: Page, name: string, colorIndex: number): Promise<void> {
  await win.getByTestId("space-add").click();
  await win.getByTestId("space-name-input").fill(name);
  await win.getByTestId("space-color").nth(colorIndex).click();
  await win.getByTestId("space-save").click();
  await expect(win.getByTestId("space-name")).toHaveText(name, { timeout: 2_000 });
}

/** 右键第 `index` 个 Space 圆点，打开编辑面板。 */
async function editSpace(win: Page, index: number): Promise<void> {
  await win.getByTestId("space-dot").nth(index).click({ button: "right" });
  await expect(win.getByTestId("space-name-input")).toBeVisible();
}

/** 等到内容区挂载的页面 URL 恰好为 `url`。 */
async function expectAttached(app: ElectronApplication, url: string, timeoutMs = 1_000): Promise<void> {
  const ok = await waitUntilAsync(timeoutMs, async () => (await attachedUrls(app)).join() === url);
  expect(ok, `内容区应当显示 ${url}，实际 ${JSON.stringify(await attachedUrls(app))}`).toBe(true);
}

/** 主进程里某个 partition 的全部 Cookie。 */
async function partitionCookies(app: ElectronApplication, partition: string): Promise<unknown[]> {
  return app.evaluate(({ session }, p) => session.fromPartition(p).cookies.get({}), partition);
}

test("N1 R1.S5 在侧边栏新建 Space：2 个圆点、第 2 个激活，名称为 Work，标签列表为空", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);

    await win.getByTestId("space-add").click();
    await win.getByTestId("space-name-input").fill("Work");
    const third = win.getByTestId("space-color").nth(2);
    await expect(third).toHaveAttribute("data-color", "#f97316");
    await third.click();
    const t0 = now();
    await win.getByTestId("space-save").click();

    await expectAllWithin(t0, 2_000, "R1.S5", async () => {
      const s = await uiState(win);
      const ok =
        s.dots.length === 2 &&
        s.dots[1]!.active &&
        !s.dots[0]!.active &&
        s.spaceName === "Work" &&
        s.rows.length === 0 &&
        s.empty === "没有打开的标签";
      return ok || JSON.stringify(s);
    });
  } finally {
    await app.close();
  }
});

test("N1 R2.S3 在侧边栏改名：右键 Personal 改成 Home 并保存，名称显示 Home 且落库", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);

    await editSpace(win, 0);
    await win.getByTestId("space-name-input").fill("Home");
    await expect(win.getByTestId("space-save")).toHaveText("保存");
    await win.getByTestId("space-save").click();

    await expect(win.getByTestId("space-name")).toHaveText("Home", { timeout: 2_000 });
    expect(queryDb(home, "SELECT name FROM spaces")).toEqual([{ name: "Home" }]);
  } finally {
    await app.close();
  }
});

test("N1 R3.S3 每个 Space 记住自己的激活标签：切回 Personal 显示 /a，再切到 Work 显示 /b", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/a"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Page A"], { timeout: 2_000 });
    await createSpace(win, "Work", 2);
    await openNewTab(win, server.input("/b"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Page B"], { timeout: 2_000 });

    /** 标签列表恰好为 `title`，且内容区挂载的页面为 `url`。 */
    const shows = (title: string, url: string) => async () => {
      const s = await uiState(win);
      const attached = await attachedUrls(app);
      const ok = s.rows.length === 1 && s.rows[0]!.text === title && attached.join() === url;
      return ok || JSON.stringify({ rows: s.rows, attached });
    };
    let t0 = now();
    await win.getByTestId("space-dot").nth(0).click();
    await expectAllWithin(t0, 1_000, "R3.S3 切回 Personal", shows("Page A", server.url("/a")));

    t0 = now();
    await win.getByTestId("space-dot").nth(1).click();
    await expectAllWithin(t0, 1_000, "R3.S3 切到 Work", shows("Page B", server.url("/b")));
  } finally {
    await app.close();
  }
});

test("N1 R5.S5 在侧边栏删除 Space 并清掉 Cookie：只剩 Personal，Work 的 partition 没有 Cookie", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await createSpace(win, "Work", 2);
    await openNewTab(win, server.input("/set-cookie"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Set Cookie"], { timeout: 2_000 });
    const partition = `persist:space-${spaceIdByName(home, "Work")}`;
    // 先确认 Cookie 真的写进了 Work 的 partition，否则「删除后为空」证明不了什么
    expect(await waitUntilAsync(2_000, async () => (await partitionCookies(app, partition)).length > 0)).toBe(true);

    await editSpace(win, 1);
    await win.getByTestId("space-delete").click();
    const t0 = now();
    await win.getByTestId("confirm-ok").click();

    await expectAllWithin(t0, 2_000, "R5.S5", async () => {
      const s = await uiState(win);
      const cookies = await partitionCookies(app, partition);
      const ok = s.dots.length === 1 && s.spaceName === "Personal" && cookies.length === 0;
      return ok || JSON.stringify({ dots: s.dots.length, spaceName: s.spaceName, cookies: cookies.length });
    });
  } finally {
    await app.close();
  }
});

test("N1 R5.S6 取消确认不删除：1 秒后仍有 2 个圆点，库里仍有 Work", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await createSpace(win, "Work", 2);

    await editSpace(win, 1);
    await win.getByTestId("space-delete").click();
    await win.getByTestId("confirm-cancel").click();

    await win.waitForTimeout(1_000);
    await expect(win.getByTestId("space-dot")).toHaveCount(2);
    expect(queryDb(home, "SELECT name FROM spaces WHERE name = 'Work'")).toEqual([{ name: "Work" }]);
  } finally {
    await app.close();
  }
});

test("N1 R7.S1 另一个 Space 看不到 Cookie：Work 里为 cookie: ，回到 Personal 新开为 cookie: k=v", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/set-cookie"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Set Cookie"], { timeout: 2_000 });

    await createSpace(win, "Work", 2);
    await openNewTab(win, server.input("/echo-cookie"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Echo Cookie"], { timeout: 2_000 });
    await expectAttached(app, server.url("/echo-cookie"), 2_000);
    expect(await attachedText(app)).toBe("cookie: ");

    await win.getByTestId("space-dot").nth(0).click();
    await expect(win.getByTestId("tab-row")).toHaveText(["Set Cookie"], { timeout: 1_000 });
    await openNewTab(win, server.input("/echo-cookie"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Set Cookie", "Echo Cookie"], { timeout: 2_000 });
    await expectAttached(app, server.url("/echo-cookie"), 2_000);
    expect(await attachedText(app)).toBe("cookie: k=v");
  } finally {
    await app.close();
  }
});

test("N1 CR5 删除 Space 时清空 HTTP 缓存：Work 的缓存大小删除前大于 0，删除后为 0", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await createSpace(win, "Work", 2);
    await openNewTab(win, server.input("/cached"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Cached"], { timeout: 2_000 });
    const partition = `persist:space-${spaceIdByName(home, "Work")}`;
    const cacheSize = () => app.evaluate(({ session }, p) => session.fromPartition(p).getCacheSize(), partition);
    expect(await waitUntilAsync(2_000, async () => (await cacheSize()) > 0), "删除前应当已有缓存").toBe(true);

    await editSpace(win, 1);
    await win.getByTestId("space-delete").click();
    await win.getByTestId("confirm-ok").click();
    await expect(win.getByTestId("space-dot")).toHaveCount(1, { timeout: 2_000 });
    expect(await waitUntilAsync(2_000, async () => (await cacheSize()) === 0), `删除后缓存应为 0，实际 ${await cacheSize()}`).toBe(true);
  } finally {
    await app.close();
  }
});
