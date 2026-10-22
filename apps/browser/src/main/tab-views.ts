// 标签页面视图：按 alterd 的快照创建、挂载、销毁 WebContentsView，并把页面事件回传给 alterd
import { session, WebContentsView, type BrowserWindow } from "electron";

import type { ShellEvent, ShellState, TabView } from "@alter/protocol";

import { SIDEBAR_WIDTH, type NavAction, type NavState } from "../shared/shell";

export class TabViews {
  private readonly views = new Map<string, WebContentsView>();
  /** 当前挂在窗口上的标签（激活标签） */
  private attached: string | undefined;
  /** 分屏时与激活标签并排挂载的另一个标签 */
  private partner: string | undefined;
  /** 分屏时显示在左边的标签（即 splitGroup 的值） */
  private left: string | undefined;
  /** 浮层（命令栏、Peek）打开期间不挂载页面：非空时 apply 只维护视图，不挂到窗口上 */
  private readonly suppressedBy = new Set<"commandBar" | "peek">();
  /** 最近一次 apply 的快照，恢复挂载时用 */
  private lastState: ShellState | undefined;
  /** 上一份快照里的 Space，用来发现被删除的 Space */
  private spaces = new Set<string>();
  /**
   * 关闭屏障：用户已关闭、关闭结果还没到的标签（固定标签等 shell.discard，普通标签等它从快照消失）。
   * 屏障期间不为它建视图：这时到达的快照还是关闭之前的。
   */
  /** 标签 → 尚未收到关闭结果的关闭次数（同一固定标签可能连续关闭多次，每次各有一条 shell.discard） */
  private readonly closing = new Map<string, number>();

  constructor(
    private readonly win: BrowserWindow,
    private readonly send: (event: ShellEvent) => void,
    private readonly onNav: (nav: NavState) => void,
    /** 用户按住 Shift 点击了链接（`new-window` 且没有窗口特性）：交给 Peek 预览 */
    private readonly onPeek?: (url: string, tab: TabView) => void,
  ) {
    win.on("resize", () => this.layout());
  }

  /**
   * 让视图与快照一致：销毁已不存在或页面已卸载（`discarded`）的标签的视图，清空已删除 Space 的数据，
   * 只挂载当前 Space 的激活标签（没有视图就新建并加载；已卸载的不加载）。
   */
  apply(state: ShellState): void {
    this.lastState = state;
    const alive = new Set(state.tabs.map((t) => t.id));
    for (const id of [...this.closing.keys()]) {
      if (!alive.has(id)) this.closing.delete(id);
    }
    const keep = new Set(state.tabs.filter((t) => !t.discarded && !this.closing.has(t.id)).map((t) => t.id));
    for (const id of [...this.views.keys()]) {
      if (!keep.has(id)) this.closeView(id);
    }
    // 被删除的 Space：它的视图已在上面随标签一起关掉，再清空它的 Cookie / storage 与 HTTP 缓存。
    // 磁盘上的 partition 目录由 alterd 下次启动时删除（这时 Chromium 可能还持有文件）
    const live = new Set(state.spaces.map((s) => s.id));
    for (const id of this.spaces) {
      if (!live.has(id)) {
        // clearStorageData 不含 HTTP 缓存，缓存要另外清
        const ses = session.fromPartition(`persist:space-${id}`);
        Promise.all([ses.clearStorageData(), ses.clearCache()]).catch((e: unknown) =>
          console.error(`[tab-views] 清空 Space ${id} 的数据失败：${String(e)}`),
        );
      }
    }
    this.spaces = live;
    const found = state.tabs.find((t) => t.spaceId === state.activeSpaceId && t.active);
    const active = found && !found.discarded && !this.closing.has(found.id) ? found : undefined;
    // TaskSpace 托管的标签总要有活的页面（agent 要对它发 CDP），不挂到窗口也建视图
    for (const tab of state.tabs) {
      if (tab.taskSpaceId !== undefined && keep.has(tab.id) && !this.views.has(tab.id)) this.create(tab);
    }
    if (active && !this.views.has(active.id)) this.create(active);
    // 分屏：激活标签所在组的另一个标签并排挂载
    const partner = active?.splitGroup
      ? state.tabs.find(
          (t) => t.splitGroup === active.splitGroup && t.id !== active.id && !t.discarded && !this.closing.has(t.id),
        )
      : undefined;
    if (partner && !this.views.has(partner.id)) this.create(partner);
    if (this.suppressedBy.size > 0) this.attach(undefined);
    else this.attach(active?.id, partner?.id, partner ? (active?.splitGroup ?? undefined) : undefined);
  }

  /** 浮层打开时摘下页面视图，全部浮层关闭后按最近的快照重新挂载。 */
  setSuppressed(by: "commandBar" | "peek", suppressed: boolean): void {
    if (suppressed) this.suppressedBy.add(by);
    else this.suppressedBy.delete(by);
    if (this.suppressedBy.size > 0) this.attach(undefined);
    else if (this.lastState) this.apply(this.lastState);
  }

  /**
   * 用户关闭了某个标签：立即关掉它的视图（旧页面不再有事件、弹窗），并设关闭屏障，
   * 直到关闭结果到达前不再为它建视图。
   */
  closingTab(tabId: string): void {
    this.closing.set(tabId, (this.closing.get(tabId) ?? 0) + 1);
    this.closeView(tabId);
  }

  /** 标签是否在关闭屏障中。 */
  isClosing(tabId: string): boolean {
    return this.closing.has(tabId);
  }

  /**
   * alterd 的 shell.discard：固定标签的一次关闭结果到了。全部关闭结果都到了才撤掉屏障，
   * 之后的快照按 `discarded` 决定是否建视图。
   */
  discard(tabId: string): void {
    const pending = (this.closing.get(tabId) ?? 1) - 1;
    if (pending > 0) this.closing.set(tabId, pending);
    else this.closing.delete(tabId);
    this.closeView(tabId);
  }

  /** 从窗口卸下并关闭某个标签的视图；没有视图时什么也不做。 */
  private closeView(tabId: string): void {
    const view = this.views.get(tabId);
    if (!view) return;
    if (this.attached === tabId) this.attach(undefined);
    else if (this.partner === tabId) this.attach(this.attached);
    view.webContents.close();
    this.views.delete(tabId);
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
    // 不挂到窗口的视图也要有尺寸（截图、布局），且不能被节流（agent 在等它）
    view.setBounds({ x: 0, y: 0, width: 1280, height: 800 });
    wc.setBackgroundThrottling(false);
    const update = (fields: Omit<Extract<ShellEvent, { type: "shell.event.tabUpdated" }>, "type" | "tabId">) => {
      // 只上报该标签当前登记的视图的事件：已关掉的旧视图迟到的事件不能把 url 改回去
      if (this.views.get(tab.id) !== view) return;
      this.send({ type: "shell.event.tabUpdated", tabId: tab.id, ...fields });
    };
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
    wc.setWindowOpenHandler(({ url, disposition, features }) => {
      // 已关掉的旧视图（关闭期间）发出的弹窗不再处理
      if (this.views.get(tab.id) !== view) return { action: "deny" };
      // Shift+点击链接：Chromium 给出 new-window 且没有窗口特性（带特性的 window.open 如 OAuth 弹窗仍转为标签）。
      // agent 页面不预览
      if (disposition === "new-window" && !features && tab.taskSpaceId === undefined && this.onPeek) {
        this.onPeek(url, tab);
        return { action: "deny" };
      }
      // 其余弹窗交给 alterd 在同一 Space 新建标签（已关掉的旧视图在上面已排除）
      this.send({ type: "shell.event.openRequested", spaceId: tab.spaceId, input: url, openerTabId: tab.id });
      return { action: "deny" };
    });
    this.views.set(tab.id, view);
    // 告诉 alterd 这个视图的 CDP targetId（Electron 直接给出，不发 CDP）
    this.send({ type: "shell.event.tabUpdated", tabId: tab.id, targetId: wc.getOrCreateDevToolsTargetId() });
    // 导航被新的导航打断时 loadURL 会 reject（ERR_ABORTED），不是错误
    wc.loadURL(tab.url).catch(() => {});
  }

  /** 挂载激活标签 `id`（分屏时连同 `partner`，`left` 为左边那个）；其余页面视图从窗口上卸下。 */
  private attach(id: string | undefined, partner?: string, left?: string): void {
    if (this.attached === id && this.partner === partner && this.left === left) return;
    const want = new Set([id, partner].filter((x): x is string => x !== undefined));
    const had = new Set([this.attached, this.partner].filter((x): x is string => x !== undefined));
    for (const old of had) {
      const view = this.views.get(old);
      if (view && !want.has(old)) this.win.contentView.removeChildView(view);
    }
    this.attached = id;
    this.partner = partner;
    this.left = left;
    for (const next of want) {
      const view = this.views.get(next);
      if (view && !had.has(next)) this.win.contentView.addChildView(view);
    }
    this.layout();
    this.emitNav();
  }

  /** 内容区：单个页面占满；分屏时左右各半。 */
  private layout(): void {
    const main = this.current();
    if (!main) return;
    const [width, height] = this.win.getContentSize();
    const area = Math.max(0, width! - SIDEBAR_WIDTH);
    const other = this.partner === undefined ? undefined : this.views.get(this.partner);
    if (!other) {
      main.setBounds({ x: SIDEBAR_WIDTH, y: 0, width: area, height: height! });
      return;
    }
    const half = Math.floor(area / 2);
    const [leftView, rightView] = this.left === this.partner ? [other, main] : [main, other];
    leftView.setBounds({ x: SIDEBAR_WIDTH, y: 0, width: half, height: height! });
    rightView.setBounds({ x: SIDEBAR_WIDTH + half, y: 0, width: area - half, height: height! });
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
