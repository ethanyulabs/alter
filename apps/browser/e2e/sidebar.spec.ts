// spaces / tabs / app-shell 能力：侧边栏、标签与导航
import { spawn } from "node:child_process";
import { existsSync, statSync } from "node:fs";
import { createConnection } from "node:net";
import { join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";

import { expect, test, type ElectronApplication, type Page } from "@playwright/test";

import { dumpLogsOnFailure, launchApp, tempHome, waitUntil } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { expectAllWithin, now, uiState } from "./ui";

/** 主进程里全部 webContents 的 URL（含侧边栏页面本身）。 */
async function contentUrls(app: ElectronApplication): Promise<string[]> {
  return app.evaluate(({ webContents }) => webContents.getAllWebContents().map((w) => w.getURL()));
}

/** 主窗口当前挂载（内容区正在显示）的标签页面 URL；没有挂载时为空数组。 */
async function attachedUrls(app: ElectronApplication): Promise<string[]> {
  return app.evaluate(({ BrowserWindow, WebContentsView }) =>
    BrowserWindow.getAllWindows()
      .flatMap((w) => w.contentView.children)
      .filter((v) => v instanceof WebContentsView)
      .map((v) => v.webContents.getURL()),
  );
}

/** 等到侧边栏连上 alterd（显示出 Space 名称）。 */
async function ready(win: Page): Promise<void> {
  await expect(win.getByTestId("space-name")).toHaveText("Personal", { timeout: 3_000 });
}

/** 用「新标签」按钮打开一个地址。 */
async function openNewTab(win: Page, input: string): Promise<void> {
  await win.getByTestId("new-tab").click();
  await win.getByTestId("address-input").fill(input);
  await win.getByTestId("address-input").press("Enter");
}

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// Playwright 要求第一个参数写成对象解构，即使不用任何 fixture
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

test("R4.S2 侧边栏显示默认 Space：Space 名称为 Personal，没有标签，内容区显示没有打开的标签", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await expect(win.getByTestId("space-name")).toHaveText("Personal", { timeout: 3_000 });
    await expect(win.getByTestId("tab-row")).toHaveCount(0);
    await expect(win.getByTestId("empty-content")).toHaveText("没有打开的标签");
  } finally {
    await app.close();
  }
});

test("R6.S1 从侧边栏新建标签：列表恰好 1 行且为激活样式，内容区加载该 URL", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/a"));

    await expect(win.getByTestId("tab-row")).toHaveCount(1, { timeout: 2_000 });
    await expect(win.getByTestId("tab-row")).toHaveClass(/active/);
    expect(await waitUntilAsync(2_000, async () => (await contentUrls(app)).includes(server.url("/a")))).toBe(true);
  } finally {
    await app.close();
  }
});

test("R8.S1 标题显示并落库：侧边栏显示 Page A，tabs.title 为 Page A", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/a"));

    await expect(win.getByTestId("tab-row")).toHaveText("Page A", { timeout: 2_000 });
    const db = new DatabaseSync(join(home, "alter.db"), { readOnly: true });
    try {
      const rows = db.prepare("SELECT title FROM tabs").all();
      expect(rows).toEqual([{ title: "Page A" }]);
    } finally {
      db.close();
    }
  } finally {
    await app.close();
  }
});

test("R7.S1 当前标签导航：在地址框打开 /b，该标签显示 /b，列表仍然恰好 1 行", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/a"));
    await expect(win.getByTestId("tab-row")).toHaveText("Page A", { timeout: 2_000 });

    await win.getByTestId("address-input").fill(server.input("/b"));
    await win.getByTestId("address-input").press("Enter");

    expect(await waitUntilAsync(2_000, async () => (await contentUrls(app)).includes(server.url("/b")))).toBe(true);
    await expect(win.getByTestId("tab-row")).toHaveCount(1);
  } finally {
    await app.close();
  }
});

test("R9.S1 点击切换：点 Page A 后内容区显示 /a，Page A 为激活样式", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/a"));
    await expect(win.getByTestId("tab-row")).toHaveText("Page A", { timeout: 2_000 });
    await openNewTab(win, server.input("/b"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Page A", "Page B"], { timeout: 2_000 });

    await win.getByTestId("tab-row").filter({ hasText: "Page A" }).click();

    expect(await waitUntilAsync(1_000, async () => (await attachedUrls(app)).join() === server.url("/a"))).toBe(true);
    await expect(win.getByTestId("tab-row").filter({ hasText: "Page A" })).toHaveClass(/active/);
    await expect(win.getByTestId("tab-row").filter({ hasText: "Page B" })).not.toHaveClass(/active/);
  } finally {
    await app.close();
  }
});

test("N1 R9.S2 关闭最后一个标签：列表为空，内容区显示没有打开的标签", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/a"));
    await expect(win.getByTestId("tab-row")).toHaveText("Page A", { timeout: 2_000 });

    await win.getByTestId("tab-row").hover();
    const t0 = now();
    await win.getByTestId("tab-close").click();

    await expectAllWithin(t0, 1_000, "R9.S2", async () => {
      const s = await uiState(win);
      return (s.rows.length === 0 && s.empty === "没有打开的标签") || JSON.stringify(s);
    });
    expect(await attachedUrls(app)).toEqual([]);
  } finally {
    await app.close();
  }
});

test("R12.S1 后退与前进：前进起初不可点，后退到 /a 后前进可点，再前进回到 /b", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/a"));
    await expect(win.getByTestId("tab-row")).toHaveText("Page A", { timeout: 2_000 });
    await win.getByTestId("address-input").fill(server.input("/b"));
    await win.getByTestId("address-input").press("Enter");
    await expect(win.getByTestId("tab-row")).toHaveText("Page B", { timeout: 2_000 });

    await expect(win.getByTestId("nav-forward")).toBeDisabled();
    await win.getByTestId("nav-back").click();
    expect(await waitUntilAsync(2_000, async () => (await attachedUrls(app)).join() === server.url("/a"))).toBe(true);
    await expect(win.getByTestId("nav-forward")).toBeEnabled();

    await win.getByTestId("nav-forward").click();
    expect(await waitUntilAsync(2_000, async () => (await attachedUrls(app)).join() === server.url("/b"))).toBe(true);
  } finally {
    await app.close();
  }
});

test("R12.S2 刷新：点刷新后 2 秒内 /count 的请求次数从 1 变为 2", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/count"));
    await expect(win.getByTestId("tab-row")).toHaveText("Count", { timeout: 2_000 });
    expect(server.hits("/count")).toBe(1);

    await win.getByTestId("nav-reload").click();
    expect(await waitUntilAsync(2_000, async () => server.hits("/count") === 2)).toBe(true);
  } finally {
    await app.close();
  }
});

/** 在新的 ALTER_HOME 里开 /a、/b、/c 三个标签，点 Page B，正常退出后重启；返回重启后的 app 与窗口。 */
async function restartWithThreeTabs(): Promise<{ app: ElectronApplication; win: Page }> {
  const home = tempHome();
  const first = await launchApp({ ALTER_HOME: home });
  try {
    const win = await first.firstWindow();
    await ready(win);
    for (const path of ["/a", "/b", "/c"]) {
      await openNewTab(win, server.input(path));
    }
    await expect(win.getByTestId("tab-row")).toHaveText(["Page A", "Page B", "Page C"], { timeout: 3_000 });
    await win.getByTestId("tab-row").filter({ hasText: "Page B" }).click();
    await expect(win.getByTestId("tab-row").filter({ hasText: "Page B" })).toHaveClass(/active/);
  } finally {
    await first.close();
  }
  const app = await launchApp({ ALTER_HOME: home });
  return { app, win: await app.firstWindow() };
}

test("R11.S1 顺序与激活标签恢复：重启后依次显示 A、B、C，Page B 激活，内容区为 /b", async () => {
  const { app, win } = await restartWithThreeTabs();
  try {
    await expect(win.getByTestId("tab-row")).toHaveText(["Page A", "Page B", "Page C"], { timeout: 3_000 });
    await expect(win.getByTestId("tab-row").filter({ hasText: "Page B" })).toHaveClass(/active/);
    expect(await waitUntilAsync(2_000, async () => (await attachedUrls(app)).join() === server.url("/b"))).toBe(true);
  } finally {
    await app.close();
  }
});

test("R11.S2 未激活的标签不预先加载：重启后只加载了 /b，点 Page C 后才加载 /c", async () => {
  const { app, win } = await restartWithThreeTabs();
  try {
    await expect(win.getByTestId("tab-row")).toHaveCount(3, { timeout: 3_000 });
    expect(await waitUntilAsync(2_000, async () => (await contentUrls(app)).includes(server.url("/b")))).toBe(true);
    const urls = await contentUrls(app);
    expect(urls).not.toContain(server.url("/a"));
    expect(urls).not.toContain(server.url("/c"));

    await win.getByTestId("tab-row").filter({ hasText: "Page C" }).click();
    expect(await waitUntilAsync(2_000, async () => (await attachedUrls(app)).join() === server.url("/c"))).toBe(true);
  } finally {
    await app.close();
  }
});

/** 仓库内构建出的 alterd（`bun run e2e` 会先 cargo build）。 */
const ALTERD = resolve(__dirname, "../../../target/debug/alterd");

/** 不启动 app，直接拉起 alterd 并经 shell.sock 开 n 个标签（/n/0 … /n/<n-1>），然后正常结束它。 */
async function seedTabs(home: string, n: number): Promise<void> {
  const alterd = spawn(ALTERD, ["run"], { env: { ...process.env, ALTER_HOME: home }, stdio: "ignore" });
  try {
    expect(await waitUntil(3_000, () => existsSync(join(home, "alterd.json")))).toBe(true);
    const socket = createConnection(join(home, "shell.sock"));
    socket.setEncoding("utf8");
    let buffer = "";
    let latest: { spaces: { id: string }[]; tabs: unknown[] } | undefined;
    socket.on("data", (chunk: string) => {
      buffer += chunk;
      let nl: number;
      while ((nl = buffer.indexOf("\n")) >= 0) {
        latest = JSON.parse(buffer.slice(0, nl));
        buffer = buffer.slice(nl + 1);
      }
    });
    expect(await waitUntilAsync(2_000, async () => latest !== undefined)).toBe(true);
    const spaceId = latest!.spaces[0]!.id;
    for (let i = 0; i < n; i++) {
      socket.write(`${JSON.stringify({ type: "shell.event.openRequested", spaceId, input: server.input(`/n/${i}`) })}\n`);
    }
    expect(await waitUntilAsync(5_000, async () => latest?.tabs.length === n)).toBe(true);
    socket.end();
  } finally {
    const exited = new Promise((r) => alterd.once("exit", r));
    alterd.kill("SIGTERM");
    await exited;
  }
}

test("SC2 重启恢复 100 个标签：从 alterd 写出健康文件起 1000 ms 内侧边栏渲染出 100 行", async () => {
  const home = tempHome();
  await seedTabs(home, 100);

  const app = await launchApp({ ALTER_HOME: home });
  try {
    // 起点：alterd.json 的写入时刻。Main 看到它后至多 100 ms 才判定 running，所以起点不晚于 running；
    // 终点：测试进程首次观察到 100 行的时刻。两者都是墙钟毫秒，结果只会高估。
    const health = join(home, "alterd.json");
    expect(await waitUntil(5_000, () => existsSync(health))).toBe(true);
    const start = statSync(health).mtimeMs;
    const win = await app.firstWindow();
    let end = 0;
    expect(
      await waitUntilAsync(5_000, async () => {
        if ((await win.getByTestId("tab-row").count()) !== 100) return false;
        end = Date.now();
        return true;
      }),
    ).toBe(true);
    const elapsed = Math.round(end - start);
    console.log(`SC2: alterd.json 写出 → 100 行 ${elapsed} ms`);
    expect(elapsed).toBeLessThan(1_000);
  } finally {
    await app.close();
  }
});

/** 只读查询 alter.db 里全部标签的 url。 */
function storedUrls(home: string): string[] {
  const db = new DatabaseSync(join(home, "alter.db"), { readOnly: true });
  try {
    return db.prepare("SELECT url FROM tabs ORDER BY position").all().map((r) => String(r.url));
  } finally {
    db.close();
  }
}

test("CR1 iframe 页内导航不改标签地址：打开 /frame，iframe 改 hash 后 tabs.url 仍为 /frame", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/frame"));
    await expect(win.getByTestId("tab-row")).toHaveText("Frame", { timeout: 2_000 });
    expect(await waitUntilAsync(2_000, async () => server.hits("/inner") === 1)).toBe(true);
    // 给 iframe 的 hash 导航与 tabUpdated 回写留出时间
    await new Promise((r) => setTimeout(r, 1_000));
    expect(storedUrls(home)).toEqual([server.url("/frame")]);
  } finally {
    await app.close();
  }
});

test("CR6 侧边栏显示 favicon：标签行的图标地址为测试页声明的 /favicon.png", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/a"));
    await expect(win.getByTestId("tab-favicon")).toHaveAttribute("src", server.url("/favicon.png"), {
      timeout: 2_000,
    });
    // 图片要真的加载成功（被 CSP 拦截或请求失败时 naturalWidth 为 0）
    await expect
      .poll(() => win.getByTestId("tab-favicon").evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth > 0), {
        timeout: 2_000,
      })
      .toBe(true);
  } finally {
    await app.close();
  }
});

test("CR7 错误提示：激活标签已在别处被关闭时，在当前标签打开会在地址框下方提示标签不存在", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/a"));
    await expect(win.getByTestId("tab-row")).toHaveText("Page A", { timeout: 2_000 });
    // 等页面的回写都到齐（标题之后 favicon 也会回写）：否则迟到的 tabUpdated 在归档后被拒绝，提前出现同样的提示
    await expect(win.getByTestId("tab-favicon")).toBeVisible({ timeout: 2_000 });

    // 直接在库里归档这个标签：alterd 不会因此广播，Main 仍以为它是激活标签
    const db = new DatabaseSync(join(home, "alter.db"));
    try {
      db.exec("PRAGMA busy_timeout = 2000");
      db.prepare("UPDATE tabs SET archived_at = 1").run();
    } finally {
      db.close();
    }
    // 排除干扰：页面迟到的 tabUpdated 也会触发同样的错误，先确认 1 秒内没有提示
    await new Promise((r) => setTimeout(r, 1_000));
    await expect(win.getByTestId("shell-error")).toHaveCount(0);
    await win.getByTestId("address-input").fill(server.input("/b"));
    await win.getByTestId("address-input").press("Enter");

    await expect(win.getByTestId("shell-error")).toContainText("标签不存在", { timeout: 2_000 });
  } finally {
    await app.close();
  }
});

test("R14.S1 点击新窗口链接：侧边栏依次为 Popup、Page B，Page B 激活，仍只有 1 个窗口", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/popup"));
    await expect(win.getByTestId("tab-row")).toHaveText("Popup", { timeout: 2_000 });

    // 链接在标签页面里，侧边栏的 DOM 够不到：在主进程里找到该页面并点击
    const popupUrl = server.url("/popup");
    await app.evaluate(async ({ webContents }, url) => {
      const page = webContents.getAllWebContents().find((w) => w.getURL() === url);
      await page?.executeJavaScript("document.getElementById('pop').click()", true);
    }, popupUrl);

    await expect(win.getByTestId("tab-row")).toHaveText(["Popup", "Page B"], { timeout: 2_000 });
    await expect(win.getByTestId("tab-row").filter({ hasText: "Page B" })).toHaveClass(/active/);
    expect(await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().length)).toBe(1);
  } finally {
    await app.close();
  }
});

test("R14.S2 window.open 打开 blob 页面：侧边栏依次为 Blob Opener、Blob，Blob 激活", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/blob"));
    await expect(win.getByTestId("tab-row")).toHaveText("Blob Opener", { timeout: 2_000 });

    const openerUrl = server.url("/blob");
    await app.evaluate(async ({ webContents }, url) => {
      const page = webContents.getAllWebContents().find((w) => w.getURL() === url);
      await page?.executeJavaScript(
        `window.open(URL.createObjectURL(new Blob(["<title>Blob</title>"], { type: "text/html" })))`,
        true,
      );
    }, openerUrl);

    await expect(win.getByTestId("tab-row")).toHaveText(["Blob Opener", "Blob"], { timeout: 2_000 });
    await expect(win.getByTestId("tab-row").filter({ hasText: /^Blob$/ })).toHaveClass(/active/);
  } finally {
    await app.close();
  }
});

/** 异步条件版的 waitUntil：每 50 ms 检查一次。 */
async function waitUntilAsync(timeoutMs: number, cond: () => Promise<boolean>): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await cond()) return true;
    await new Promise((r) => setTimeout(r, 50));
  }
  return cond();
}

test("N1 CR11 连发成功与被拒绝的操作：错误提示不被随后到达的快照清掉，1 秒后仍显示", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/a"));
    await expect(win.getByTestId("tab-row")).toHaveText("Page A", { timeout: 2_000 });

    await win.evaluate(async () => {
      const alter = (window as unknown as {
        alter: {
          getShellView(): Promise<{ state: { tabs: { id: string }[] } }>;
          activateTab(id: string): void;
          closeTab(id: string): void;
        };
      }).alter;
      const id = (await alter.getShellView()).state.tabs[0]!.id;
      // 第一个成功（会广播快照），第二个被拒绝（只回错误）
      alter.activateTab(id);
      alter.closeTab("01JZZZZZZZZZZZZZZZZZZZZZZZ");
    });

    await expect(win.getByTestId("shell-error")).toBeVisible({ timeout: 1_000 });
    await win.waitForTimeout(1_000);
    await expect(win.getByTestId("shell-error")).toBeVisible();
  } finally {
    await app.close();
  }
});
