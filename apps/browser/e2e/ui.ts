// N1 之后各 e2e 共用的界面辅助：等待就绪、开标签、读主进程里的视图、只读查库
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";

import { expect, type ElectronApplication, type Page } from "@playwright/test";

/** 等到侧边栏连上 alterd（显示出 Space 名称）。 */
export async function ready(win: Page, spaceName = "Personal"): Promise<void> {
  // 准备步骤，不是场景断言：构建后第一次启动 app 偶尔要好几秒，放宽等待
  await expect(win.getByTestId("space-name")).toHaveText(spaceName, { timeout: 10_000 });
}

/** 用「新标签」按钮打开一个地址。 */
export async function openNewTab(win: Page, input: string): Promise<void> {
  await win.getByTestId("new-tab").click();
  await win.getByTestId("address-input").fill(input);
  await win.getByTestId("address-input").press("Enter");
}

/** 主进程里全部 webContents 的 URL（含侧边栏页面本身）。 */
export async function contentUrls(app: ElectronApplication): Promise<string[]> {
  return app.evaluate(({ webContents }) => webContents.getAllWebContents().map((w) => w.getURL()));
}

/** 主窗口当前挂载（内容区正在显示）的标签页面 URL；没有挂载时为空数组。 */
export async function attachedUrls(app: ElectronApplication): Promise<string[]> {
  return app.evaluate(({ BrowserWindow, WebContentsView }) =>
    BrowserWindow.getAllWindows()
      .flatMap((w) => w.contentView.children)
      .filter((v) => v instanceof WebContentsView)
      .map((v) => v.webContents.getURL()),
  );
}

/** 当前挂载的标签页面的正文（`document.body.textContent`）；没有挂载时为 undefined。 */
export async function attachedText(app: ElectronApplication): Promise<string | undefined> {
  return app.evaluate(async ({ BrowserWindow, WebContentsView }) => {
    const view = BrowserWindow.getAllWindows()
      .flatMap((w) => w.contentView.children)
      .find((v) => v instanceof WebContentsView);
    if (!view) return undefined;
    return (await view.webContents.executeJavaScript("document.body?.textContent ?? ''")) as string;
  });
}

/** 每 50 ms 检查一次异步条件，直到成立或超时；返回条件最终是否成立。 */
export async function waitUntilAsync(timeoutMs: number, cond: () => Promise<boolean>): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await cond()) return true;
    await new Promise((r) => setTimeout(r, 50));
  }
  return cond();
}

/** 只读打开 `ALTER_HOME/alter.db` 执行一条查询（alterd 运行时也可以读，WAL）。 */
export function queryDb(home: string, sql: string, ...params: (string | number)[]): Record<string, unknown>[] {
  const db = new DatabaseSync(join(home, "alter.db"), { readOnly: true });
  try {
    return db.prepare(sql).all(...params) as Record<string, unknown>[];
  } finally {
    db.close();
  }
}

/** 库里名为 `name` 的 Space 的 id。 */
export function spaceIdByName(home: string, name: string): string {
  const rows = queryDb(home, "SELECT id FROM spaces WHERE name = ?", name);
  expect(rows, `库里应当恰好有一个 Space ${name}`).toHaveLength(1);
  return String(rows[0]!.id);
}

/** 侧边栏一行。 */
export interface RowState {
  /** 行上的 `data-tab-id` */
  id: string;
  text: string;
  active: boolean;
  pinned: boolean;
  /** 是否在屏幕上位于（可见的）固定分区分隔线上方 */
  aboveDivider: boolean;
}

/** 一次读出的侧边栏状态：同一时刻的 Space 圆点、Space 名称、标签行与内容区的空提示。 */
export interface UiState {
  dots: { active: boolean }[];
  spaceName: string;
  rows: RowState[];
  /** 「没有打开的标签」提示的文字；没有显示时为 null */
  empty: string | null;
}

export async function uiState(win: Page): Promise<UiState> {
  return win.evaluate(() => {
    const q = (id: string) => [...document.querySelectorAll(`[data-testid="${id}"]`)];
    const divider = document.querySelector('[data-testid="pinned-divider"]');
    // 分隔线要真的看得见：有尺寸、没有被 display / visibility / opacity 藏起来
    const dividerTop = (() => {
      if (!divider) return undefined;
      const style = getComputedStyle(divider);
      const rect = divider.getBoundingClientRect();
      // 分隔线由上边框画出：边框要有宽度、颜色不能全透明
      const border = parseFloat(style.borderTopWidth) > 0 && !/^(transparent|rgba\(.*,\s*0\))$/.test(style.borderTopColor);
      const visible =
        style.display !== "none" &&
        style.visibility !== "hidden" &&
        Number(style.opacity) > 0 &&
        rect.width > 0 &&
        border;
      return visible ? rect.top : undefined;
    })();
    return {
      dots: q("space-dot").map((d) => ({ active: d.classList.contains("active") })),
      spaceName: document.querySelector('[data-testid="space-name"]')?.textContent ?? "",
      rows: q("tab-row").map((r) => ({
        id: r.getAttribute("data-tab-id") ?? "",
        text: r.textContent ?? "",
        active: r.classList.contains("active"),
        pinned: r.getAttribute("data-pinned") === "true",
        // 按屏幕位置：整行都在分隔线上沿之上
        aboveDivider: dividerTop !== undefined && r.getBoundingClientRect().bottom <= dividerTop + 0.5,
      })),
      empty: document.querySelector('[data-testid="empty-content"]')?.textContent ?? null,
    };
  });
}

/** 单调时钟的毫秒数（不受系统校时影响）。 */
export function now(): number {
  return performance.now();
}

/**
 * 场景写「操作后 N 毫秒内 A 且 B」时：从操作前记下的 `t0`（`now()`）起，在时限内反复检查组合条件 `check`，
 * 某一次全部成立即通过；`check` 返回 true 或说明哪里不符的文字。超时后用最后一次的说明报错。
 */
export async function expectAllWithin(
  t0: number,
  ms: number,
  what: string,
  check: () => Promise<true | string>,
): Promise<void> {
  let last: true | string = "还没有检查过";
  while (now() - t0 <= ms) {
    last = await check();
    if (last === true && now() - t0 <= ms) return;
    await new Promise((r) => setTimeout(r, 30));
  }
  throw new Error(`${what}：${ms} ms 内条件没有同时成立；最后一次：${last === true ? "成立但已超时" : last}`);
}
