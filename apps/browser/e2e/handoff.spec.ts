// N3 接管与交还：横幅、交还按钮、停止 Agent（经 `alter nodejs` 驱动真实的 app）
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { chromium, expect, test, type Browser, type ElectronApplication, type Page } from "@playwright/test";

import { runScript } from "./agent";
import { dumpLogsOnFailure, launchApp, tempHome } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { now, ready, waitUntilAsync } from "./ui";

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

/** 经 alterd 的 CDP 转发端点连上 app 里的 Chromium。 */
async function connectCdp(home: string): Promise<Browser> {
  const { cdpPort, token } = JSON.parse(readFileSync(join(home, "gateway.json"), "utf8")) as { cdpPort: number; token: string };
  return chromium.connectOverCDP(`http://127.0.0.1:${cdpPort}`, {
    headers: { Authorization: `Bearer ${token}` },
    timeout: 5_000,
  });
}

/** CDP 连接里 URL 为 `url` 的页面。 */
async function pageWithUrl(browser: Browser, url: string): Promise<Page> {
  let found: Page | undefined;
  await waitUntilAsync(2_000, async () => {
    found = browser.contexts().flatMap((c) => c.pages()).find((p) => p.url() === url);
    return found !== undefined;
  });
  if (!found) throw new Error(`CDP 里没有 ${url} 的页面`);
  return found;
}

/** 叠层的状态：动作文字、光标中心、计算样式的 display。没有叠层时为 null。 */
function overlayState(page: Page) {
  return page.evaluate(() => {
    const host = document.querySelector("alter-agent-overlay");
    if (!host?.shadowRoot) return null;
    const cursor = host.shadowRoot.querySelector("[data-role=cursor]")!.getBoundingClientRect();
    return {
      action: host.shadowRoot.querySelector("[data-role=action]")!.textContent,
      cursor: { x: cursor.left + cursor.width / 2, y: cursor.top + cursor.height / 2 },
      display: getComputedStyle(host).display,
      visibility: getComputedStyle(host).visibility,
    };
  });
}

/** 按钮中心（视口坐标）。 */
function buttonCenter(page: Page, name: string) {
  return page.getByRole("button", { name }).evaluate((el) => {
    const r = el.getBoundingClientRect();
    return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  });
}

/** 新 app 里建 TaskSpace，p1 打开 /form，返回 (app, home, TaskSpace id)。 */
async function formPage() {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  await ready(await app.firstWindow());
  const r = await runScript(
    home,
    `const t = await taskSpace("demo"); await t.page("p1").goto(${JSON.stringify(server.url("/form"))}); console.log(t.id);`,
  );
  expect(r.code, r.stderr).toBe(0);
  return { app, home, id: Number(r.stdout.trim()) };
}

test("N3 R1.S2 侧边栏显示接管横幅：等待你操作：login 与「交还给 Agent」", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    // 2 秒从启动脚本之前算起（单调时钟，含 Bun 启动与建 TaskSpace，只会高估）
    const deadline = now() + 2_000;
    const left = () => Math.max(1, deadline - now());
    const script = runScript(home, `const t = await taskSpace("login"); await t.handOff();`);
    const banner = win.getByTestId("handoff-banner");
    await expect(banner).toBeVisible({ timeout: left() });
    await expect(banner).toContainText("等待你操作：login", { timeout: left() });
    await expect(banner.getByRole("button", { name: "交还给 Agent" })).toBeVisible({ timeout: left() });
    expect(now(), "横幅没有在 2 秒内出现").toBeLessThanOrEqual(deadline);
    const r = await script;
    expect(r.code, r.stderr).toBe(0);
  } finally {
    await app.close();
  }
});

test("N3 R3.S2 在侧边栏交还：横幅 1 秒内消失，takeOverTaskSpace 的 ownership 为 agent", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    const r = await runScript(home, `const t = await taskSpace("login"); await t.handOff(); console.log(t.id);`);
    expect(r.code, r.stderr).toBe(0);
    const id = Number(r.stdout.trim());
    await expect(win.getByTestId("handoff-banner")).toBeVisible({ timeout: 2_000 });

    // 1 秒从点击之前算起（单调时钟，含点击本身，只会高估）
    const deadline = now() + 1_000;
    await win.getByTestId("handoff-return").click();
    await expect(win.getByTestId("handoff-banner")).toHaveCount(0, { timeout: Math.max(1, deadline - now()) });
    expect(now(), "横幅没有在 1 秒内消失").toBeLessThanOrEqual(deadline);
    const back = await runScript(home, `console.log((await takeOverTaskSpace(${id})).ownership);`);
    expect(back.code, back.stderr).toBe(0);
    expect(back.stdout.trim()).toBe("agent");
  } finally {
    await app.close();
  }
});

test("N3 R5.S2 在侧边栏停止 Agent：横幅 1 秒内出现，之后 page.click 以 E_USER_CONTROL 失败", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    const r = await runScript(home, `const t = await taskSpace("demo"); console.log(t.id);`);
    expect(r.code, r.stderr).toBe(0);
    const id = Number(r.stdout.trim());
    const stop = win.getByTestId("stop-agent");
    await expect(stop).toBeVisible({ timeout: 2_000 });

    // 1 秒从点击之前算起（单调时钟，含点击本身，只会高估）
    const deadline = now() + 1_000;
    await stop.click();
    await expect(win.getByTestId("handoff-banner")).toBeVisible({ timeout: Math.max(1, deadline - now()) });
    expect(now(), "横幅没有在 1 秒内出现").toBeLessThanOrEqual(deadline);
    const click = await runScript(home, `await (await taskSpace(${id})).page("p1").click("body");`);
    expect(click.code).toBe(1);
    expect(click.stderr).toContain("E_USER_CONTROL");
  } finally {
    await app.close();
  }
});

test("N3 R8.S1 点击时显示光标与文字：action 为 提交表单，光标中心距 Submit 中心不超过 4 px", async () => {
  const { app, home, id } = await formPage();
  const browser = await connectCdp(home);
  try {
    const page = await pageWithUrl(browser, server.url("/form"));
    const submit = await buttonCenter(page, "Submit");
    // 1 秒从启动脚本之前算起（单调时钟，只会高估）：叠层显示且文字正确的时刻不能晚于截止时间
    const deadline = now() + 1_000;
    const script = runScript(home, `await (await taskSpace(${id})).page("p1").click("text=Submit", { actionLabel: "提交表单" });`);
    let state: Awaited<ReturnType<typeof overlayState>> = null;
    await waitUntilAsync(Math.max(1, deadline - now()), async () => {
      state = await overlayState(page);
      return state?.action === "提交表单" && state.display === "block";
    });
    expect(now(), "叠层没有在 1 秒内显示").toBeLessThanOrEqual(deadline);
    expect(state).not.toBeNull();
    const s = state!;
    expect(s.action).toBe("提交表单");
    expect([s.display, s.visibility]).toEqual(["block", "visible"]);
    expect(Math.hypot(s.cursor.x - submit.x, s.cursor.y - submit.y)).toBeLessThanOrEqual(4);
    const r = await script;
    expect(r.code, r.stderr).toBe(0);
  } finally {
    await browser.close();
    await app.close();
  }
});

test("N3 R8.S2 叠层不拦截点击：在叠层覆盖的 Submit 中心点击，正文出现 submitted:", async () => {
  const { app, home, id } = await formPage();
  const browser = await connectCdp(home);
  try {
    const page = await pageWithUrl(browser, server.url("/form"));
    const r = await runScript(home, `await (await taskSpace(${id})).page("p1").hover("text=Submit", { actionLabel: "看看按钮" });`);
    expect(r.code, r.stderr).toBe(0);
    expect(await waitUntilAsync(1_000, async () => (await overlayState(page))?.display === "block")).toBe(true);
    const submit = await buttonCenter(page, "Submit");
    await page.mouse.click(submit.x, submit.y);
    await expect(page.locator("body")).toContainText("submitted:", { timeout: 1_000 });
  } finally {
    await browser.close();
    await app.close();
  }
});

/** 在主进程里读 URL 为 `url` 的页面上叠层的 display（用户控制期间 CDP 转发端点不可用）；没有叠层时为 null。 */
function overlayDisplayViaMain(app: ElectronApplication, url: string) {
  return app.evaluate(async ({ webContents }, url) => {
    const wc = webContents.getAllWebContents().find((w) => w.getURL() === url);
    if (!wc) return null;
    return (await wc.executeJavaScript(
      `(() => { const h = document.querySelector("alter-agent-overlay"); return h ? getComputedStyle(h).display : null; })()`,
    )) as string | null;
  }, url);
}

test("N3 R8.S3 用户控制期间隐藏叠层：handOff 后 1 秒内 display 为 none", async () => {
  const { app, home, id } = await formPage();
  try {
    const url = server.url("/form");
    const r = await runScript(home, `await (await taskSpace(${id})).page("p1").hover("text=Submit", { actionLabel: "看看按钮" });`);
    expect(r.code, r.stderr).toBe(0);
    expect(await waitUntilAsync(1_000, async () => (await overlayDisplayViaMain(app, url)) === "block")).toBe(true);
    // 1 秒从启动 handOff 脚本之前算起（单调时钟，只会高估）
    const deadline = now() + 1_000;
    const off = runScript(home, `await (await taskSpace(${id})).handOff();`);
    expect(
      await waitUntilAsync(Math.max(1, deadline - now()), async () => (await overlayDisplayViaMain(app, url)) === "none"),
    ).toBe(true);
    expect(now(), "叠层没有在 1 秒内隐藏").toBeLessThanOrEqual(deadline);
    const r2 = await off;
    expect(r2.code, r2.stderr).toBe(0);
  } finally {
    await app.close();
  }
});

