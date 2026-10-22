// 主进程与渲染进程共用的 Shell 状态类型与 IPC 通道（状态本身来自 alterd 的 shell.stateChanged）
import type {
  ErrorCode,
  ImportCounts,
  ImportItem,
  ImportSource,
  ImportStatus,
  SearchItem,
  ShellState,
} from "@alter/protocol";

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

/** IPC 通道：固定 / 取消固定，载荷为 `{ tabId, pinned }`。 */
export const TAB_PIN_CHANNEL = "tab.pin";

/** IPC 通道：把某个标签与当前激活标签组成分屏 / 解散它所在的分屏，载荷为 `{ tabId }`。 */
export const TAB_SPLIT_CHANNEL = "tab.split";
/** IPC 通道：恢复一个已归档的标签，载荷为 `{ tabId }`。 */
export const TAB_RESTORE_CHANNEL = "tab.restore";
export const TAB_UNSPLIT_CHANNEL = "tab.unsplit";

export interface TabPinRequest {
  tabId: string;
  pinned: boolean;
}

/** IPC 通道：拖拽后移到目标分区（`pinned`）的第 `index` 位（不含被拖的标签本身）。 */
export const TAB_MOVE_CHANNEL = "tab.move";

export interface TabMoveRequest {
  tabId: string;
  pinned: boolean;
  index: number;
}

/** IPC 通道：对激活标签执行后退 / 前进 / 刷新，每个动作一个通道，载荷为 `{}`。 */
export type NavAction = "back" | "forward" | "reload";
export const NAV_CHANNELS: Record<NavAction, string> = {
  back: "nav.back",
  forward: "nav.forward",
  reload: "nav.reload",
};

/** IPC 通道：Space 的新建 / 编辑 / 切换 / 删除。 */
export const SPACE_CHANNELS = {
  create: "space.create",
  update: "space.update",
  activate: "space.activate",
  delete: "space.delete",
} as const;

export interface SpaceCreateRequest {
  name: string;
  color: string;
}

/** 只改给出的字段。 */
export interface SpaceUpdateRequest {
  spaceId: string;
  name?: string;
  color?: string;
}

export interface SpaceRequest {
  spaceId: string;
}

/** IPC 通道：用户在接管横幅上交还 / 在侧边栏停止 Agent，载荷为 `{ taskSpaceId }`。 */
export const TASK_SPACE_CHANNELS = {
  return: "taskSpace.return",
  stop: "taskSpace.stop",
} as const;

export interface TaskSpaceRequest {
  taskSpaceId: number;
}

/**
 * IPC 通道：命令栏。`open`（主 → 渲染）打开；`close`（渲染 → 主）用户按 Esc 关闭，主进程恢复挂载页面；
 * `search`（渲染 → 主）输入变化，转为 alterd 的 searchRequested；`results`（主 → 渲染）搜索结果；
 * `execute`（渲染 → 主）回车或点击某一项。
 */
export const COMMAND_BAR_CHANNELS = {
  open: "commandBar.open",
  close: "commandBar.close",
  search: "commandBar.search",
  results: "commandBar.results",
  execute: "commandBar.execute",
} as const;

export interface CommandSearchRequest {
  requestId: number;
  query: string;
}

export interface CommandSearchResults {
  requestId: number;
  items: SearchItem[];
}

/**
 * IPC 通道：Peek。`state`（主 → 渲染）打开时带 Peek 视图在窗口里的位置，关闭时 `open` 为假；
 * `close`、`promote`（渲染 → 主）关闭 / 在新标签打开。
 */
export const PEEK_CHANNELS = {
  state: "peek.state",
  close: "peek.close",
  promote: "peek.promote",
} as const;

export interface PeekState {
  open: boolean;
  /** Peek 视图在窗口内容区里的位置（窗口坐标） */
  bounds?: { x: number; y: number; width: number; height: number };
}

/**
 * IPC 通道：导入向导。`sources`（渲染 → 主，invoke）列出可导入的 profile；`start`（渲染 → 主）开始导入，
 * 转为 alterd 的 importRequested；`progress`（主 → 渲染）alterd 的 importProgress。
 */
export const IMPORT_CHANNELS = {
  sources: "import.sources",
  start: "import.start",
  progress: "import.progress",
} as const;

/** 各浏览器数据根目录下可导入的 profile 名称。 */
export interface ImportSources {
  chrome: string[];
  arc: string[];
}

export interface ImportStartRequest {
  requestId: number;
  source: ImportSource;
  profile: string;
  spaceId: string;
  items: ImportItem[];
}

/** 一条导入进度；最后一条的 status 为 done、partial 或 failed。 */
export interface ImportProgressView {
  requestId: number;
  importId: number;
  status: ImportStatus;
  item?: ImportItem;
  counts: ImportCounts;
  warning?: string;
  error?: string;
}

/** IPC 通道（主进程 → 渲染进程）：聚焦地址框（快捷键 / 菜单）。 */
export const ADDRESS_FOCUS_CHANNEL = "address.focus";

export interface AddressFocusRequest {
  /** 为真时清空地址框并进入新标签模式，否则选中其中文字 */
  newTab: boolean;
}

/** 渲染进程经 preload 拿到的 Shell API。 */
export interface ShellApi {
  getShellView(): Promise<ShellView>;
  onShellView(listener: (view: ShellView) => void): () => void;
  open(request: OpenRequest): void;
  activateTab(tabId: string): void;
  closeTab(tabId: string): void;
  pinTab(request: TabPinRequest): void;
  splitTab(tabId: string): void;
  restoreTab(tabId: string): void;
  unsplitTab(tabId: string): void;
  moveTab(request: TabMoveRequest): void;
  navigate(action: NavAction): void;
  createSpace(request: SpaceCreateRequest): void;
  updateSpace(request: SpaceUpdateRequest): void;
  activateSpace(spaceId: string): void;
  deleteSpace(spaceId: string): void;
  returnTaskSpace(taskSpaceId: number): void;
  onPeekState(listener: (state: PeekState) => void): () => void;
  closePeek(): void;
  promotePeek(): void;
  onCommandBarOpen(listener: () => void): () => void;
  closeCommandBar(): void;
  search(request: CommandSearchRequest): void;
  onSearchResults(listener: (results: CommandSearchResults) => void): () => void;
  executeCommand(item: SearchItem): void;
  stopTaskSpace(taskSpaceId: number): void;
  onAddressFocus(listener: (request: AddressFocusRequest) => void): () => void;
  importSources(): Promise<ImportSources>;
  startImport(request: ImportStartRequest): void;
  onImportProgress(listener: (progress: ImportProgressView) => void): () => void;
}
