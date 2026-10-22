// TaskSpace：agent 在某个 Space 里的任务空间，持有若干 Page
import type { Ownership, TaskSpaceInfo, TaskSpaceTab } from "@alter/protocol";

import type { Client } from "./client";
import { AlterError } from "./errors";
import { Page } from "./page";

export class TaskSpace {
  readonly id: number;
  readonly name: string;
  readonly spaceId: string;
  ownership: Ownership;
  /** 最近一次已知的未关闭 Page 标签 */
  pageLabels: string[];
  /** `takeOverTaskSpace` 返回的、用户交还时所在的页面标签 */
  private userPageLabel: string | null = null;

  constructor(
    private readonly client: Client,
    info: TaskSpaceInfo,
  ) {
    this.id = info.id;
    this.name = info.name;
    this.spaceId = info.spaceId;
    this.ownership = info.ownership;
    this.pageLabels = info.pages;
  }

  private call<T = unknown>(method: string, params: Record<string, unknown> = {}): Promise<T> {
    return this.client.call<T>(method, { id: this.id, ...params });
  }

  page(label: string): Page {
    return new Page(this.client, this.id, label);
  }
  async pages(): Promise<Page[]> {
    const info = await this.client.call<TaskSpaceInfo>("taskSpace.resume", { id: this.id });
    this.pageLabels = info.pages;
    return info.pages.map((l) => this.page(l));
  }
  async tabs(): Promise<TaskSpaceTab[]> {
    return (await this.call<{ tabs: TaskSpaceTab[] }>("taskSpace.tabs")).tabs;
  }
  async newPage(): Promise<Page> {
    const { label } = await this.call<{ label: string }>("taskSpace.newPage");
    return this.page(label);
  }
  async adopt(tabId: string): Promise<Page> {
    const { label } = await this.call<{ label: string }>("taskSpace.adopt", { tabId });
    return this.page(label);
  }
  async release(label: string): Promise<void> {
    await this.call("taskSpace.release", { label });
  }
  finish(opts: { keep?: string[] } = {}): Promise<{ closed: string[]; kept: string[] }> {
    return this.call("taskSpace.finish", { keep: opts.keep ?? [] });
  }
  /** 把控制权交给用户：侧边栏出现接管横幅，之后的页面命令回 `E_USER_CONTROL`，直到用户交还。 */
  async handOff(): Promise<void> {
    await this.call("taskSpace.handOff");
    this.ownership = "user";
  }

  /**
   * 等用户交还控制权（收到 `ownership` 为 `agent` 的 `taskSpace.ownershipChanged`）。
   * 超时抛 `E_TIMEOUT`。先订阅再查询当前状态，订阅前已交还的也能立即返回。
   */
  async waitForControl(opts: { timeoutMs?: number } = {}): Promise<void> {
    const timeoutMs = opts.timeoutMs ?? 600_000;
    let off = () => {};
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      await new Promise<void>((resolve, reject) => {
        off = this.client.subscribe(({ method, params }) => {
          if (method === "taskSpace.ownershipChanged" && params["id"] === this.id && params["ownership"] === "agent") {
            resolve();
          }
        });
        timer = setTimeout(
          () => reject(new AlterError("E_TIMEOUT", `${timeoutMs} ms 内用户没有交还 TaskSpace ${this.id}`)),
          timeoutMs,
        );
        // 这次请求也让 Gateway 把这个连接登记为该 TaskSpace 的订阅者
        this.client
          .call<TaskSpaceInfo>("taskSpace.resume", { id: this.id })
          .then((info) => {
            if (info.ownership === "agent") resolve();
          }, reject);
      });
      this.ownership = "agent";
    } finally {
      off();
      if (timer !== undefined) clearTimeout(timer);
    }
  }

  /** `takeOverTaskSpace` 接回时用户所在的页面；没有时为 null。 */
  userPage(): Page | null {
    return this.userPageLabel === null ? null : this.page(this.userPageLabel);
  }

  /** 由 `takeOverTaskSpace` 填写。 */
  setUserPage(label: string | null): void {
    this.userPageLabel = label;
  }
  async cdp<T = unknown>(method: string, params: Record<string, unknown> = {}): Promise<T> {
    return (await this.call<{ result: T }>("taskSpace.cdp", { method, params })).result;
  }
}
