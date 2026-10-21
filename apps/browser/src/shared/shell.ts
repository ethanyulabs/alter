// 主进程与渲染进程共用的 Shell 状态类型与 IPC 通道（状态本身来自 alterd 的 shell.stateChanged）
import type { ErrorCode, ShellState } from "@alter/protocol";

/** 侧边栏宽度（px）；主进程按它计算标签视图的位置。 */
export const SIDEBAR_WIDTH = 240;

/** IPC 通道：渲染进程 invoke 取当前值，主进程 send 推送变化。 */
export const SHELL_VIEW_CHANNEL = "shell.state";

/** 激活标签的导航状态。 */
export interface NavState {
  canGoBack: boolean;
  canGoForward: boolean;
  url: string;
}

/** 渲染进程看到的全部 Shell 状态。 */
export interface ShellView {
  /** alterd 的最新快照；还没连上时为 null */
  state: ShellState | null;
  nav: NavState;
  /** 最近一次被 alterd 拒绝的操作 */
  error?: { code: ErrorCode; message: string };
}

/** IPC 通道：地址框提交。`newTab` 为真时新建标签，否则在当前激活标签打开。 */
export const SHELL_OPEN_CHANNEL = "shell.open";

export interface OpenRequest {
  input: string;
  newTab: boolean;
}

/** IPC 通道：激活 / 关闭侧边栏里的标签，载荷为 `{ tabId }`。 */
export const TAB_ACTIVATE_CHANNEL = "tab.activate";
export const TAB_CLOSE_CHANNEL = "tab.close";

export interface TabRequest {
  tabId: string;
}

/** IPC 通道：对激活标签执行后退 / 前进 / 刷新，每个动作一个通道，载荷为 `{}`。 */
export type NavAction = "back" | "forward" | "reload";
export const NAV_CHANNELS: Record<NavAction, string> = {
  back: "nav.back",
  forward: "nav.forward",
  reload: "nav.reload",
};

/** 渲染进程经 preload 拿到的 Shell API。 */
export interface ShellApi {
  getShellView(): Promise<ShellView>;
  onShellView(listener: (view: ShellView) => void): () => void;
  open(request: OpenRequest): void;
  activateTab(tabId: string): void;
  closeTab(tabId: string): void;
  navigate(action: NavAction): void;
}
