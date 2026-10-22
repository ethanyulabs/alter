// app-shell 能力：应用菜单与快捷键（N1 R12，N4 MODIFIED 后为 R5：tab.new 改为打开命令栏）（e2e 按不了真实按键，在主进程里点菜单项驱动同一段处理代码）
import { expect, test, type ElectronApplication, type Page } from "@playwright/test";

import { dumpLogsOnFailure, launchApp } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { expectAllWithin, now, openNewTab, ready, uiState, waitUntilAsync, type RowState } from "./ui";

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

/** R12 的菜单项 id 与快捷键。 */
const SHORTCUTS: Record<string, string> = {
  "tab.new": "CmdOrCtrl+T",
  "tab.close": "CmdOrCtrl+W",
  "address.focus": "CmdOrCtrl+L",
  "nav.reload": "CmdOrCtrl+R",
  "nav.back": "CmdOrCtrl+[",
  "nav.forward": "CmdOrCtrl+]",
  ...Object.fromEntries(Array.from({ length: 9 }, (_, i) => [`space.${i + 1}`, `CmdOrCtrl+${i + 1}`])),
  "tab.next": "Ctrl+Tab",
  "tab.prev": "Ctrl+Shift+Tab",
  "tab.togglePin": "CmdOrCtrl+Shift+D",
};

/** 在主进程点击应用菜单里的某一项；菜单项不存在时抛错。 */
async function clickMenu(app: ElectronApplication, id: string): Promise<void> {
  await app.evaluate(({ Menu }, itemId) => {
    const item = Menu.getApplicationMenu()?.getMenuItemById(itemId);
    if (!item) throw new Error(`应用菜单里没有 ${itemId}`);
    item.click();
  }, id);
}

/** 各行的 [标题, 是否激活, 是否固定]。 */
function rowsOf(rows: RowState[]): [string, boolean, boolean][] {
  return rows.map((r) => [r.text, r.active, r.pinned]);
}

function row(win: Page, title: string) {
  return win.getByTestId("tab-row").filter({ hasText: title });
}

async function openAll(win: Page, paths: string[], titles: string[]): Promise<void> {
  for (const path of paths) await openNewTab(win, server.input(path));
  await expect(win.getByTestId("tab-row")).toHaveText(titles, { timeout: 3_000 });
}

test("N4 R5.S1 菜单项与快捷键：每个菜单项都存在且 accelerator 与表中完全相同", async () => {
  const app = await launchApp();
  try {
    await ready(await app.firstWindow());
    const actual = await app.evaluate(({ Menu }, ids) => {
      const menu = Menu.getApplicationMenu();
      return Object.fromEntries(ids.map((id) => [id, menu?.getMenuItemById(id)?.accelerator ?? null]));
    }, Object.keys(SHORTCUTS));
    expect(actual).toEqual(SHORTCUTS);
  } finally {
    await app.close();
  }
});

test("N4 R5.S2 关闭激活标签：点 tab.close 后只剩 Page A 且为激活样式", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a", "/b"], ["Page A", "Page B"]);
    await expect(row(win, "Page B")).toHaveClass(/active/);

    const t0 = now();
    await clickMenu(app, "tab.close");
    await expectAllWithin(t0, 1_000, "R12.S2", async () => {
      const got = rowsOf((await uiState(win)).rows);
      return JSON.stringify(got) === JSON.stringify([["Page A", true, false]]) || JSON.stringify(got);
    });
  } finally {
    await app.close();
  }
});

test("N4 R5.S3 按序号切换 Space：space.1 切到 Personal，space.3 不存在时不变", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await win.getByTestId("space-add").click();
    await win.getByTestId("space-name-input").fill("Work");
    await win.getByTestId("space-save").click();
    await expect(win.getByTestId("space-name")).toHaveText("Work", { timeout: 2_000 });

    const t0 = now();
    await clickMenu(app, "space.1");
    await expectAllWithin(t0, 1_000, "R5.S3 space.1", async () => {
      const name = await win.getByTestId("space-name").textContent();
      return name === "Personal" || `底部为 ${name}`;
    });
    // 再点 space.3（不存在）：点击返回后至少 1 秒，底部仍为 Personal
    await clickMenu(app, "space.3");
    await win.waitForTimeout(1_000);
    await expect(win.getByTestId("space-name")).toHaveText("Personal");
  } finally {
    await app.close();
  }
});

test("N4 R5.S4 循环切换标签：Page C 激活时 tab.next 到 Page A，tab.prev 回到 Page C", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a", "/b", "/c"], ["Page A", "Page B", "Page C"]);
    await expect(row(win, "Page C")).toHaveClass(/active/);

    const activeTitle = async () => (await uiState(win)).rows.find((r) => r.active)?.text;
    const t0 = now();
    await clickMenu(app, "tab.next");
    await expectAllWithin(t0, 1_000, "R5.S4 tab.next", async () => {
      const t = await activeTitle();
      return t === "Page A" || `激活的是 ${t}`;
    });
    const t1 = now();
    await clickMenu(app, "tab.prev");
    await expectAllWithin(t1, 1_000, "R5.S4 tab.prev", async () => {
      const t = await activeTitle();
      return t === "Page C" || `激活的是 ${t}`;
    });
  } finally {
    await app.close();
  }
});

test("N4 R5.S5 新标签与聚焦地址框：tab.new 后键入地址回车新开 Page B，address.focus 聚焦地址框", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a"], ["Page A"]);

    await clickMenu(app, "tab.new");
    await win.keyboard.type(server.input("/b"));
    const t0 = now();
    await win.keyboard.press("Enter");
    await expectAllWithin(t0, 2_000, "R12.S5", async () => {
      const got = rowsOf((await uiState(win)).rows).map(([t, a]) => [t, a]);
      const want = [
        ["Page A", false],
        ["Page B", true],
      ];
      return JSON.stringify(got) === JSON.stringify(want) || JSON.stringify(got);
    });

    // R5 的表：聚焦地址框并选中其中文字（场景没写时限，按 1 秒、从点击之前计）
    const t1 = now();
    await clickMenu(app, "address.focus");
    await expectAllWithin(t1, 1_000, "R5.S5 address.focus", async () => {
      const ok = await win.evaluate(() => {
        const el = document.activeElement;
        return (
          el instanceof HTMLInputElement &&
          el.getAttribute("data-testid") === "address-input" &&
          el.value.length > 0 &&
          el.selectionStart === 0 &&
          el.selectionEnd === el.value.length
        );
      });
      return ok || "地址框没有聚焦并选中文字";
    });
  } finally {
    await app.close();
  }
});

test("N4 R5.S6 快捷键固定激活标签：tab.togglePin 后 Page B 排到最前并在固定分区", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a", "/b"], ["Page A", "Page B"]);

    const t0 = now();
    await clickMenu(app, "tab.togglePin");
    await expectAllWithin(t0, 1_000, "R12.S6", async () => {
      const s = await uiState(win);
      const ok =
        s.rows.map((r) => r.text).join() === "Page B,Page A" && s.rows[0]!.pinned && s.rows[0]!.aboveDivider;
      return ok || JSON.stringify(s.rows);
    });
  } finally {
    await app.close();
  }
});

test("N4 R5.S7 快捷键刷新：nav.reload 后 2 秒内 /count 的请求次数从 1 变为 2", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/count"], ["Count"]);
    expect(server.hits("/count")).toBe(1);

    const t0 = now();
    await clickMenu(app, "nav.reload");
    await expectAllWithin(t0, 2_000, "R5.S7", async () => server.hits("/count") === 2 || `请求次数 ${server.hits("/count")}`);
  } finally {
    await app.close();
  }
});

test("N1 CR28 没有激活标签时：tab.next 激活第一行，tab.prev 激活最后一行", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a", "/b"], ["Page A", "Page B"]);
    // 两个都固定后依次关闭：Space 里只剩已卸载的标签，没有激活标签。
    // 每次先确认要固定的那一行已是激活样式，再按快捷键（Main 按它收到的快照找激活标签）
    for (const title of ["Page B", "Page A"]) {
      await row(win, title).click();
      await expect(row(win, title)).toHaveClass(/active/, { timeout: 1_000 });
      await clickMenu(app, "tab.togglePin");
      await expect(row(win, title)).toHaveAttribute("data-pinned", "true", { timeout: 1_000 });
    }
    for (const title of ["Page A", "Page B"]) {
      await row(win, title).hover();
      await row(win, title).getByTestId("tab-close").click();
    }
    await expect(win.getByTestId("empty-content")).toHaveText("没有打开的标签", { timeout: 1_000 });
    const titles = await win.getByTestId("tab-row").allTextContents();

    await clickMenu(app, "tab.next");
    await expect(row(win, titles[0]!)).toHaveClass(/active/, { timeout: 1_000 });
    // 再回到没有激活标签的状态，检查 tab.prev
    await row(win, titles[0]!).hover();
    await row(win, titles[0]!).getByTestId("tab-close").click();
    await expect(win.getByTestId("empty-content")).toHaveText("没有打开的标签", { timeout: 1_000 });
    await clickMenu(app, "tab.prev");
    await expect(row(win, titles.at(-1)!)).toHaveClass(/active/, { timeout: 1_000 });
  } finally {
    await app.close();
  }
});

test("N1 R12 菜单后退与前进：nav.back 回到 /a，nav.forward 回到 /b", async () => {
  const app = await launchApp();
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openAll(win, ["/a"], ["Page A"]);
    // 在当前标签里导航到 /b
    await win.getByTestId("address-input").fill(server.input("/b"));
    await win.getByTestId("address-input").press("Enter");
    await expect(win.getByTestId("tab-row")).toHaveText(["Page B"], { timeout: 2_000 });

    const attached = () =>
      app.evaluate(({ BrowserWindow, WebContentsView }) =>
        BrowserWindow.getAllWindows()
          .flatMap((w) => w.contentView.children)
          .filter((v) => v instanceof WebContentsView)
          .map((v) => v.webContents.getURL())
          .join(),
      );
    await clickMenu(app, "nav.back");
    expect(await waitUntilAsync(2_000, async () => (await attached()) === server.url("/a"))).toBe(true);
    await clickMenu(app, "nav.forward");
    expect(await waitUntilAsync(2_000, async () => (await attached()) === server.url("/b"))).toBe(true);
  } finally {
    await app.close();
  }
});
