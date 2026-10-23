// N5 导入向导：从侧边栏菜单打开，选来源、profile、目标 Space 与类别，显示进度与计数
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { dumpLogsOnFailure, launchApp, tempHome } from "./app";
import { DAY_MS, standardCookies, standardProfile, writeCookies } from "./fixtures/profile";
import { startServer } from "./fixtures/server";
import { attachedText, attachedUrls, expectAllWithin, now, openNewTab, queryDb, ready, spaceIdByName, waitUntilAsync } from "./ui";

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
    // Cookie 默认勾选（RC N1 R7.S2）；本场景只导书签与历史
    await item("cookies").uncheck();

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

/** 打开导入向导，选 Chrome 的 Default 与目标 Space（已有的 Space 名）。返回定位类别复选框的函数。 */
async function openWizard(win: import("@playwright/test").Page, space: string) {
  await win.getByTestId("sidebar-menu").click();
  await win.getByTestId("menu-import").click();
  await win.getByTestId("import-source").selectOption("chrome");
  const profile = win.getByTestId("import-profile");
  await expect(profile.locator("option")).toHaveText(["Default"], { timeout: 2_000 });
  await profile.selectOption({ label: "Default" });
  await win.getByTestId("import-space").selectOption({ label: space });
  return (value: string) => win.locator(`[data-testid="import-item"][value="${value}"]`);
}

test("RC N1 R7.S2 Cookie 可选且默认勾选，旁边说明会请求钥匙串授权", async () => {
  const home = tempHome();
  const chrome = mkdtempSync(join(tmpdir(), "alter-chrome-"));
  standardProfile(join(chrome, "Default"));
  const app = await launchApp({ ALTER_HOME: home, ALTER_CHROME_DIR: chrome, ALTER_ARC_DIR: join(home, "no-arc") });
  try {
    const win = await app.firstWindow();
    await ready(win);
    const item = await openWizard(win, "Personal");
    await expect(item("cookies")).toBeEnabled();
    await expect(item("cookies")).toBeChecked();
    await expect(win.getByTestId("import-wizard")).toContainText("macOS 会请求钥匙串授权");
  } finally {
    await app.close();
  }
});

test("RC N1 R7.S3 拒绝授权时 5 秒内显示 部分完成、书签 3、Cookie 0 与原因", async () => {
  const home = tempHome();
  const chrome = mkdtempSync(join(tmpdir(), "alter-chrome-"));
  standardProfile(join(chrome, "Default"));
  standardCookies(join(chrome, "Default"));
  const app = await launchApp({
    ALTER_HOME: home,
    ALTER_CHROME_DIR: chrome,
    ALTER_ARC_DIR: join(home, "no-arc"),
    ALTER_KEYCHAIN_FAKE: "denied",
  });
  try {
    const win = await app.firstWindow();
    await ready(win);
    const item = await openWizard(win, "Personal");
    for (const v of ["bookmarks", "history", "cookies"]) await item(v).check();
    const t0 = now();
    await win.getByTestId("import-start").click();
    await expectAllWithin(t0, 5_000, "R7.S3", () =>
      win.evaluate((): true | string => {
        const text = document.querySelector('[data-testid="import-result"]')?.textContent ?? "";
        for (const want of ["部分完成", "书签 3", "Cookie 0", "未获得钥匙串授权，Cookie 未导入"]) {
          if (!text.includes(want)) return `结果区没有「${want}」：${text}`;
        }
        return true;
      }),
    );
  } finally {
    await app.close();
  }
});

test("RC N1 R6.S1 只有目标 Space 带上导入的 Cookie：Work 的 /echo-cookie 有 sid=abc，Personal 没有", async () => {
  const server = await startServer();
  const home = tempHome();
  const chrome = mkdtempSync(join(tmpdir(), "alter-chrome-"));
  standardCookies(join(chrome, "Default"));
  const app = await launchApp({
    ALTER_HOME: home,
    ALTER_CHROME_DIR: chrome,
    ALTER_ARC_DIR: join(home, "no-arc"),
    ALTER_KEYCHAIN_FAKE: "peanuts",
    // Main 不回报时别等满 10 秒
    ALTER_IMPORT_COOKIE_TIMEOUT_MS: "3000",
  });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await win.getByTestId("space-add").click();
    await win.getByTestId("space-name-input").fill("Work");
    await win.getByTestId("space-save").click();
    await expect(win.getByTestId("space-name")).toHaveText("Work", { timeout: 2_000 });

    const item = await openWizard(win, "Work");
    await item("bookmarks").uncheck();
    await item("history").uncheck();
    await expect(item("cookies")).toBeChecked();
    const t0 = now();
    await win.getByTestId("import-start").click();
    // Main 写入后必须回报 cookiesImported：没有回报时 alterd 超时，结果是「部分完成」而不是「导入完成」
    await expectAllWithin(t0, 5_000, "R6.S1", () =>
      win.evaluate((): true | string => {
        const text = document.querySelector('[data-testid="import-result"]')?.textContent ?? "";
        for (const want of ["导入完成", "Cookie 2", "跳过 0"]) {
          if (!text.includes(want)) return `结果区没有「${want}」：${text}`;
        }
        return document.querySelector('[data-testid="import-done"]') ? true : "还没有「完成」按钮";
      }),
    );
    await win.getByTestId("import-done").click();
    // host-only 的 Cookie 写入后仍是 host-only，不能变成对子域名也生效的 domain Cookie
    const partition = `persist:space-${spaceIdByName(home, "Work")}`;
    const sid = await app.evaluate(
      async ({ session }, p) => (await session.fromPartition(p).cookies.get({ name: "sid" }))[0],
      partition,
    );
    expect(sid?.hostOnly, `sid：${JSON.stringify(sid)}`).toBe(true);
    expect(sid?.domain, `sid：${JSON.stringify(sid)}`).toBe("127.0.0.1");

    /** 在当前 Space 新开 /echo-cookie，返回页面里看到的请求头 Cookie。 */
    const echoed = async (): Promise<string> => {
      await openNewTab(win, server.input("/echo-cookie"));
      const url = server.url("/echo-cookie");
      const shown = await waitUntilAsync(2_000, async () => (await attachedUrls(app)).join() === url);
      expect(shown, `内容区应当显示 ${url}，实际 ${JSON.stringify(await attachedUrls(app))}`).toBe(true);
      return (await attachedText(app)) ?? "";
    };
    const inWork = await echoed();
    expect(inWork, `Work 的页面：${inWork}`).toContain("sid=abc");

    await win.getByTestId("space-dot").nth(0).click();
    await expect(win.getByTestId("space-name")).toHaveText("Personal", { timeout: 2_000 });
    const inPersonal = await echoed();
    expect(inPersonal, `Personal 的页面：${inPersonal}`).not.toContain("sid=abc");
  } finally {
    await app.close();
    await server.close();
  }
});

test("RC N1 T12 host-only 的 Cookie 写入后仍是 host-only，__Host- 前缀的也能写入", async () => {
  const home = tempHome();
  const chrome = mkdtempSync(join(tmpdir(), "alter-chrome-"));
  const later = Date.now() + 30 * DAY_MS;
  writeCookies(join(chrome, "Default"), [
    { host: "app.example.test", name: "only", value: "1", expiresMs: later },
    { host: ".example.test", name: "wide", value: "2", expiresMs: later },
    { host: "app.example.test", name: "__Host-sid", value: "3", expiresMs: later, secure: true },
  ]);
  const app = await launchApp({
    ALTER_HOME: home,
    ALTER_CHROME_DIR: chrome,
    ALTER_ARC_DIR: join(home, "no-arc"),
    ALTER_KEYCHAIN_FAKE: "peanuts",
  });
  try {
    const win = await app.firstWindow();
    await ready(win);
    const item = await openWizard(win, "Personal");
    await item("bookmarks").uncheck();
    await item("history").uncheck();
    await win.getByTestId("import-start").click();
    await expect(win.getByTestId("import-result")).toContainText("Cookie 3", { timeout: 5_000 });
    const partition = `persist:space-${spaceIdByName(home, "Personal")}`;
    const all = await app.evaluate(
      async ({ session }, p) =>
        (await session.fromPartition(p).cookies.get({})).map((c) => ({ name: c.name, domain: c.domain, hostOnly: c.hostOnly })),
      partition,
    );
    const by = (name: string) => all.find((c) => c.name === name);
    expect(by("only"), JSON.stringify(all)).toEqual({ name: "only", domain: "app.example.test", hostOnly: true });
    expect(by("wide"), JSON.stringify(all)).toEqual({ name: "wide", domain: ".example.test", hostOnly: false });
    expect(by("__Host-sid")?.hostOnly, JSON.stringify(all)).toBe(true);
  } finally {
    await app.close();
  }
});
