// 标签页面视图：按 alterd 的快照创建、挂载、销毁 WebContentsView，并把页面事件回传给 alterd
import { WebContentsView, type BrowserWindow } from "electron";

import type { ShellEvent, ShellState, TabView } from "@alter/protocol";

import { SIDEBAR_WIDTH, type NavAction, type NavState } from "../shared/shell";

export class TabViews {
  private readonly views = new Map<string, WebContentsView>();
  /** 当前挂在窗口上的标签 */
  private attached: string | undefined;

  constructor(
    private readonly win: BrowserWindow,
    private readonly send: (event: ShellEvent) => void,
    private readonly onNav: (nav: NavState) => void,
  ) {
    win.on("resize", () => this.layout());
  }

  /** 让视图与快照一致：销毁已不存在的标签，只挂载当前 Space 的激活标签（没有视图就新建并加载）。 */
  apply(state: ShellState): void {
    const alive = new Set(state.tabs.map((t) => t.id));
    for (const [id, view] of this.views) {
      if (!alive.has(id)) {
        if (this.attached === id) this.attach(undefined);
        view.webContents.close();
        this.views.delete(id);
      }
    }
    const active = state.tabs.find((t) => t.spaceId === state.activeSpaceId && t.active);
    if (active && !this.views.has(active.id)) this.create(active);
    this.attach(active?.id);
  }

  /** 在已有标签的视图里打开 URL（alterd 的 shell.navigate）。 */
  navigate(tabId: string, url: string): void {
    this.views.get(tabId)?.webContents.loadURL(url).catch(() => {});
  }

  /** 对当前挂载的标签执行后退 / 前进 / 刷新；不可后退或前进时什么也不做。 */
  navAction(action: NavAction): void {
    const wc = this.current()?.webContents;
    if (!wc) return;
    if (action === "back" && wc.navigationHistory.canGoBack()) wc.navigationHistory.goBack();
    else if (action === "forward" && wc.navigationHistory.canGoForward()) wc.navigationHistory.goForward();
    else if (action === "reload") wc.reload();
  }

  /** 当前挂载的视图。 */
  current(): WebContentsView | undefined {
    return this.attached === undefined ? undefined : this.views.get(this.attached);
  }

  private create(tab: TabView): void {
    const view = new WebContentsView({
      webPreferences: { partition: `persist:space-${tab.spaceId}`, sandbox: true },
    });
    const wc = view.webContents;
    const update = (fields: Omit<Extract<ShellEvent, { type: "shell.event.tabUpdated" }>, "type" | "tabId">) =>
      this.send({ type: "shell.event.tabUpdated", tabId: tab.id, ...fields });
    wc.on("page-title-updated", (_e, title) => update({ title }));
    wc.on("page-favicon-updated", (_e, favicons) => {
      const faviconUrl = favicons[0];
      if (faviconUrl) update({ faviconUrl });
    });
    const navigated = (url: string) => {
      update({ url });
      if (this.attached === tab.id) this.emitNav();
    };
    // did-navigate 只对主框架触发；did-navigate-in-page 对 iframe 也触发，只认主框架
    wc.on("did-navigate", (_e, url) => navigated(url));
    wc.on("did-navigate-in-page", (_e, url, isMainFrame) => {
      if (isMainFrame) navigated(url);
    });
    // 网页弹窗（target=_blank、window.open）不开新窗口，交给 alterd 在同一 Space 新建标签
    wc.setWindowOpenHandler(({ url }) => {
      this.send({ type: "shell.event.openRequested", spaceId: tab.spaceId, input: url });
      return { action: "deny" };
    });
    this.views.set(tab.id, view);
    // 导航被新的导航打断时 loadURL 会 reject（ERR_ABORTED），不是错误
    wc.loadURL(tab.url).catch(() => {});
  }

  private attach(id: string | undefined): void {
    if (this.attached === id) return;
    const prev = this.current();
    if (prev) this.win.contentView.removeChildView(prev);
    this.attached = id;
    const next = this.current();
    if (next) {
      this.win.contentView.addChildView(next);
      this.layout();
    }
    this.emitNav();
  }

  private layout(): void {
    const view = this.current();
    if (!view) return;
    const [width, height] = this.win.getContentSize();
    view.setBounds({ x: SIDEBAR_WIDTH, y: 0, width: Math.max(0, width! - SIDEBAR_WIDTH), height: height! });
  }

  private emitNav(): void {
    const wc = this.current()?.webContents;
    this.onNav({
      canGoBack: wc?.navigationHistory.canGoBack() ?? false,
      canGoForward: wc?.navigationHistory.canGoForward() ?? false,
      url: wc?.getURL() ?? "",
    });
  }
}
