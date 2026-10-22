// Page：TaskSpace 里一个带永久标签（p1、p2…）的页面；每个方法对应 Gateway 的一个 page.* 调用
import type { Receipt } from "@alter/protocol";

import type { Client } from "./client";

/** 元素动作的选项。 */
export interface ActionOptions {
  /** 定位元素的等待上限，默认 5000 */
  timeoutMs?: number;
  /** 3–6 个词的动作描述，显示在页面上 */
  actionLabel?: string;
}

export interface GotoOptions {
  waitUntil?: "load" | "domcontentloaded";
  timeoutMs?: number;
}

/** `waitFor` 的条件：URL（子串）、选择器、页内函数（返回真值即满足）或等一段时间。 */
export type WaitCondition = { url: string } | { selector: string } | { fn: string | ((...a: never[]) => unknown) } | { ms: number };

export class Page {
  constructor(
    private readonly client: Client,
    readonly taskSpaceId: number,
    readonly label: string,
  ) {}

  private call<T = unknown>(method: string, params: Record<string, unknown> = {}): Promise<T> {
    return this.client.call<T>(method, { id: this.taskSpaceId, label: this.label, ...params });
  }

  private action(method: string, selector: string, extra: Record<string, unknown>, opts: ActionOptions = {}): Promise<Receipt> {
    return this.call<Receipt>(method, { selector, ...extra, ...opts });
  }

  goto(url: string, opts: GotoOptions = {}): Promise<Receipt> {
    return this.call("page.goto", { url, ...opts });
  }
  reload(opts: GotoOptions = {}): Promise<Receipt> {
    return this.call("page.reload", { ...opts });
  }
  info(): Promise<{ url: string; title: string; loading: boolean; dialog?: { type: string; message: string } }> {
    return this.call("page.info");
  }
  async url(): Promise<string> {
    return (await this.info()).url;
  }
  async title(): Promise<string> {
    return (await this.info()).title;
  }
  async snapshot(opts: { scope?: "full_page" | "subtree"; root?: string } = {}): Promise<string> {
    return (await this.call<{ text: string }>("page.snapshot", { ...opts })).text;
  }
  screenshot(opts: { path: string; fullPage?: boolean }): Promise<{ path: string; width: number; height: number }> {
    return this.call("page.screenshot", { ...opts });
  }

  click(selector: string, opts?: ActionOptions): Promise<Receipt> {
    return this.action("page.click", selector, {}, opts);
  }
  dblclick(selector: string, opts?: ActionOptions): Promise<Receipt> {
    return this.action("page.dblclick", selector, {}, opts);
  }
  hover(selector: string, opts?: ActionOptions): Promise<Receipt> {
    return this.action("page.hover", selector, {}, opts);
  }
  focus(selector: string, opts?: ActionOptions): Promise<Receipt> {
    return this.action("page.focus", selector, {}, opts);
  }
  fill(selector: string, value: string, opts?: ActionOptions): Promise<Receipt> {
    return this.action("page.fill", selector, { value }, opts);
  }
  press(selector: string, key: string, opts?: ActionOptions): Promise<Receipt> {
    return this.action("page.press", selector, { value: key }, opts);
  }
  selectOption(selector: string, value: string | string[], opts?: ActionOptions): Promise<Receipt> {
    return this.action("page.selectOption", selector, { value }, opts);
  }
  dragAndDrop(source: string, target: string, opts?: ActionOptions): Promise<Receipt> {
    return this.action("page.dragAndDrop", source, { value: target }, opts);
  }
  setInputFiles(selector: string, files: string | string[], opts?: ActionOptions): Promise<Receipt> {
    return this.action("page.setInputFiles", selector, { value: files }, opts);
  }

  /** 在页面里执行函数，返回可序列化的结果。 */
  async evaluate<T = unknown>(fn: string | ((arg: never) => unknown), arg?: unknown): Promise<T> {
    const source = typeof fn === "function" ? fn.toString() : fn;
    return (await this.call<{ value: T }>("page.evaluate", { fn: source, arg })).value;
  }

  async waitFor(cond: WaitCondition, opts: { timeoutMs?: number } = {}): Promise<boolean> {
    const [kind, arg] =
      "url" in cond
        ? ["url", cond.url]
        : "selector" in cond
          ? ["selector", cond.selector]
          : "fn" in cond
            ? ["fn", typeof cond.fn === "function" ? cond.fn.toString() : cond.fn]
            : ["ms", cond.ms];
    return (await this.call<{ matched: boolean }>("page.waitFor", { kind, arg, ...opts })).matched;
  }

  dialog(opts: { accept: boolean; promptText?: string }): Promise<Record<string, never>> {
    return this.call("page.dialog", { ...opts });
  }
  close(): Promise<Record<string, never>> {
    return this.call("page.close");
  }
  /** 原始指针事件（视口坐标）。 */
  readonly mouse = {
    move: (x: number, y: number): Promise<Receipt> => this.call("page.mouse", { op: "move", args: { x, y } }),
    down: (x: number, y: number, opts: { button?: string; clickCount?: number } = {}): Promise<Receipt> =>
      this.call("page.mouse", { op: "down", args: { x, y, ...opts } }),
    up: (x: number, y: number, opts: { button?: string; clickCount?: number } = {}): Promise<Receipt> =>
      this.call("page.mouse", { op: "up", args: { x, y, ...opts } }),
    click: (x: number, y: number, opts: { button?: string; clickCount?: number } = {}): Promise<Receipt> =>
      this.call("page.mouse", { op: "click", args: { x, y, ...opts } }),
    wheel: (x: number, y: number, deltaX: number, deltaY: number): Promise<Receipt> =>
      this.call("page.mouse", { op: "wheel", args: { x, y, deltaX, deltaY } }),
  };
  /** 原始键盘事件：`press` / `down` / `up` 用键名（如 `Enter`），`type` 整段插入文字。 */
  readonly keyboard = {
    down: (key: string): Promise<Receipt> => this.call("page.keyboard", { op: "down", args: { key } }),
    up: (key: string): Promise<Receipt> => this.call("page.keyboard", { op: "up", args: { key } }),
    press: (key: string): Promise<Receipt> => this.call("page.keyboard", { op: "press", args: { key } }),
    type: (text: string): Promise<Receipt> => this.call("page.keyboard", { op: "type", args: { text } }),
  };
  /** 在页面里 fetch（带页面的 Cookie）；`saveAs`（绝对路径）时正文写进文件，返回 `path`。 */
  fetch(
    url: string,
    options: Record<string, unknown> = {},
    opts: { saveAs?: string } = {},
  ): Promise<{ ok: boolean; status: number; headers: Record<string, string>; body?: string; path?: string }> {
    return this.call("page.fetch", { url, options, ...opts });
  }
  async cdp<T = unknown>(method: string, params: Record<string, unknown> = {}): Promise<T> {
    return (await this.call<{ result: T }>("page.cdp", { method, params })).result;
  }

  /** 等这个页面的下一次 `popup` / `download` / `dialog` 事件。 */
  waitForEvent(event: "popup" | "download" | "dialog", opts: { timeoutMs?: number } = {}): Promise<Record<string, unknown>> {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        off();
        reject(new Error(`等待 ${event} 事件超时`));
      }, opts.timeoutMs ?? 30_000);
      const off = this.client.subscribe(({ method, params }) => {
        if (method === `page.${event}` && params["id"] === this.taskSpaceId && params["label"] === this.label) {
          clearTimeout(timer);
          off();
          resolve(params);
        }
      });
    });
  }
}
