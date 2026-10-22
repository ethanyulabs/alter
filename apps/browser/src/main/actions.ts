// 用户动作：Renderer 的 IPC 与应用菜单（快捷键）共用这一组函数，e2e 点菜单项测到的就是按键时走的代码
import type { SearchItem, ShellEvent, ShellState, TabView } from "@alter/protocol";

import type {
  NavAction,
  OpenRequest,
  SpaceCreateRequest,
  SpaceUpdateRequest,
  TabMoveRequest,
  TabPinRequest,
} from "../shared/shell";

export interface ActionContext {
  /** alterd 的最新快照；还没连上时为 null */
  state(): ShellState | null;
  /** 发给 alterd 的 Shell 事件 */
  send(event: ShellEvent): void;
  /** 对当前挂载的标签执行后退 / 前进 / 刷新 */
  nav(action: NavAction): void;
  /** 让 Renderer 聚焦地址框；`newTab` 为真时进入新标签模式 */
  focusAddress(newTab: boolean): void;
  /** 摘下页面视图并让 Renderer 打开命令栏 */
  openCommandBar(): void;
  /** 用户关闭了某个标签（发出 tabClosed 之前调用）：Main 立即关掉它的视图并设关闭屏障 */
  closing(tabId: string): void;
  /** 标签是否在关闭屏障中（已发出关闭、结果未到） */
  isClosing(tabId: string): boolean;
}

export type Actions = ReturnType<typeof createActions>;

export function createActions(ctx: ActionContext) {
  /** 当前 Space 的标签，按侧边栏顺序（固定在前）。 */
  const spaceTabs = (): TabView[] => {
    const s = ctx.state();
    return s ? s.tabs.filter((t) => t.spaceId === s.activeSpaceId) : [];
  };
  /** 当前激活标签；已发出关闭、结果未到的标签不算（这时手上的快照还是关闭之前的）。 */
  const activeTab = (): TabView | undefined => {
    const tab = spaceTabs().find((t) => t.active);
    return tab && !ctx.isClosing(tab.id) ? tab : undefined;
  };
  const close = (tabId: string): void => {
    ctx.closing(tabId);
    ctx.send({ type: "shell.event.tabClosed", tabId });
  };
  const activate = (tabId: string): void => {
    ctx.send({ type: "shell.event.tabActivated", tabId });
  };

  return {
    open(request: OpenRequest): void {
      const state = ctx.state();
      if (!state) return;
      const spaceId = state.activeSpaceId;
      // 不是新标签模式且当前 Space 有激活标签时在该标签打开，否则新建标签
      const tabId = request.newTab ? undefined : activeTab()?.id;
      ctx.send({ type: "shell.event.openRequested", spaceId, input: request.input, ...(tabId ? { tabId } : {}) });
    },
    activateTab(tabId: string): void {
      activate(tabId);
    },
    /** 把 `tabId` 与当前激活标签组成分屏，激活标签在左边。 */
    splitWithActive(tabId: string): void {
      const active = activeTab();
      if (!active || active.id === tabId) return;
      ctx.send({ type: "shell.event.splitRequested", tabIds: [active.id, tabId] });
    },
    unsplit(tabId: string): void {
      ctx.send({ type: "shell.event.splitDissolved", tabId });
    },
    closeTab(tabId: string): void {
      close(tabId);
    },
    /** 关闭当前 Space 的激活标签；没有时什么也不做。 */
    closeActiveTab(): void {
      const tab = activeTab();
      if (tab) close(tab.id);
    },
    pinTab({ tabId, pinned }: TabPinRequest): void {
      ctx.send({ type: "shell.event.tabPinned", tabId, pinned });
    },
    /** 固定或取消固定激活标签。 */
    togglePinActive(): void {
      const tab = activeTab();
      if (tab) ctx.send({ type: "shell.event.tabPinned", tabId: tab.id, pinned: !tab.pinned });
    },
    moveTab({ tabId, pinned, index }: TabMoveRequest): void {
      ctx.send({ type: "shell.event.tabMoved", tabId, pinned, index });
    },
    /** 激活列表里相邻的标签（`delta` 为 +1 / -1），首尾循环。 */
    cycleTab(delta: number): void {
      const tabs = spaceTabs();
      if (tabs.length === 0) return;
      const active = activeTab();
      const i = active ? tabs.indexOf(active) : -1;
      // 没有激活标签（只剩已卸载的标签）时：向后从第一行开始，向前从最后一行开始
      const from = i >= 0 ? i : delta > 0 ? -1 : tabs.length;
      const next = tabs[(((from + delta) % tabs.length) + tabs.length) % tabs.length]!;
      activate(next.id);
    },
    createSpace({ name, color }: SpaceCreateRequest): void {
      ctx.send({ type: "shell.event.spaceCreated", name, color });
    },
    updateSpace({ spaceId, name, color }: SpaceUpdateRequest): void {
      ctx.send({
        type: "shell.event.spaceUpdated",
        spaceId,
        ...(name !== undefined ? { name } : {}),
        ...(color !== undefined ? { color } : {}),
      });
    },
    activateSpace(spaceId: string): void {
      ctx.send({ type: "shell.event.spaceActivated", spaceId });
    },
    /** 切到第 `n` 个 Space（从 1 开始）；不存在时什么也不做。 */
    activateSpaceAt(n: number): void {
      const space = ctx.state()?.spaces[n - 1];
      if (space) ctx.send({ type: "shell.event.spaceActivated", spaceId: space.id });
    },
    deleteSpace(spaceId: string): void {
      ctx.send({ type: "shell.event.spaceDeleted", spaceId });
    },
    /** 用户把 TaskSpace 交还给 agent。 */
    returnTaskSpace(taskSpaceId: number): void {
      ctx.send({ type: "shell.event.ownershipReturned", taskSpaceId });
    },
    /** 用户主动接管（停止 Agent）。 */
    stopTaskSpace(taskSpaceId: number): void {
      ctx.send({ type: "shell.event.userTookControl", taskSpaceId });
    },
    nav(action: NavAction): void {
      ctx.nav(action);
    },
    focusAddress(newTab: boolean): void {
      ctx.focusAddress(newTab);
    },
    openCommandBar(): void {
      ctx.openCommandBar();
    },
    /** 执行命令栏的一项：标签切到它的 Space 并激活，Space 切过去，书签、历史与「打开」在当前 Space 新建标签。 */
    executeCommand(item: SearchItem): void {
      const state = ctx.state();
      if (!state) return;
      if (item.kind === "tab" && item.tabId) {
        if (item.spaceId && item.spaceId !== state.activeSpaceId) {
          ctx.send({ type: "shell.event.spaceActivated", spaceId: item.spaceId });
        }
        activate(item.tabId);
      } else if (item.kind === "space" && item.spaceId) {
        ctx.send({ type: "shell.event.spaceActivated", spaceId: item.spaceId });
      } else {
        const input = item.kind === "history" || item.kind === "bookmark" ? item.url : item.input;
        if (input) ctx.send({ type: "shell.event.openRequested", spaceId: state.activeSpaceId, input });
      }
    },
  };
}
