// Peek：Shift+点击链接时，在当前页面之上的浮层里加载它，不新建标签。浮层期间摘下页面视图，
// Renderer 在内容区画遮罩与「在新标签打开」；Esc（页面里或 Renderer 里）或点遮罩关闭
import { WebContentsView, type BrowserWindow } from "electron";

import type { ShellEvent, ShellState, TabView } from "@alter/protocol";

import { PEEK_CHANNELS, SIDEBAR_WIDTH, type PeekState } from "../shared/shell";
import { partitionFor, type TabViews } from "./tab-views";

/** Peek 视图的标记：测试经 `app.evaluate` 从 `globalThis` 读取，以区分 Peek 与标签页面 */
const marked = new WeakSet<WebContentsView>();
(globalThis as { __alterPeekViews?: WeakSet<WebContentsView> }).__alterPeekViews = marked;

/** Peek 占内容区的比例 */
const RATIO = 0.8;

export class Peek {
  private view: WebContentsView | undefined;
  /** 打开 Peek 的标签所在的 Space：「在新标签打开」时在这里新建 */
  private spaceId: string | undefined;

  constructor(
    private readonly win: BrowserWindow,
    private readonly tabViews: () => TabViews | undefined,
    private readonly send: (event: ShellEvent) => void,
  ) {
    win.on("resize", () => this.layout());
  }

  open(url: string, tab: TabView): void {
    this.close();
    const view = new WebContentsView({ webPreferences: { partition: partitionFor(tab.spaceId), sandbox: true } });
    marked.add(view);
    this.view = view;
    this.spaceId = tab.spaceId;
    // 页面里按 Esc 也关闭
    view.webContents.on("before-input-event", (event, input) => {
      if (input.type === "keyDown" && input.key === "Escape") {
        event.preventDefault();
        this.close();
      }
    });
    // Peek 里再打开的窗口转为标签
    view.webContents.setWindowOpenHandler(({ url: next }) => {
      if (this.spaceId) this.send({ type: "shell.event.openRequested", spaceId: this.spaceId, input: next });
      return { action: "deny" };
    });
    this.tabViews()?.setSuppressed("peek", true);
    this.win.contentView.addChildView(view);
    this.layout();
    view.webContents.loadURL(url).catch(() => {});
  }

  /** 快照到达：当前 Space 不再是 Peek 所在的 Space，或那个 Space 已删除时关闭 Peek。 */
  sync(state: ShellState): void {
    if (!this.view) return;
    const space = this.spaceId;
    if (state.activeSpaceId !== space || !state.spaces.some((s) => s.id === space)) this.close();
  }

  /** 关闭 Peek，恢复挂载页面。 */
  close(): void {
    const view = this.view;
    if (!view) return;
    this.view = undefined;
    this.win.contentView.removeChildView(view);
    view.webContents.close();
    this.tabViews()?.setSuppressed("peek", false);
    this.notify({ open: false });
  }

  /** 在新标签打开 Peek 当前的 URL，并关闭 Peek。 */
  promote(): void {
    const view = this.view;
    if (!view || !this.spaceId) return;
    const url = view.webContents.getURL();
    const spaceId = this.spaceId;
    this.close();
    if (url) this.send({ type: "shell.event.openRequested", spaceId, input: url });
  }

  private layout(): void {
    if (!this.view) return;
    const [width, height] = this.win.getContentSize();
    const areaW = Math.max(0, width! - SIDEBAR_WIDTH);
    const w = Math.round(areaW * RATIO);
    const h = Math.round(height! * RATIO);
    const bounds = { x: SIDEBAR_WIDTH + Math.round((areaW - w) / 2), y: Math.round((height! - h) / 2), width: w, height: h };
    this.view.setBounds(bounds);
    this.notify({ open: true, bounds });
  }

  private notify(state: PeekState): void {
    this.win.webContents.send(PEEK_CHANNELS.state, state);
  }
}
