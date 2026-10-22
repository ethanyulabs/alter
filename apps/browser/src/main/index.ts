// Electron 主进程入口：创建主窗口，托管 alterd
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { app, BrowserWindow, ipcMain, Menu } from "electron";

import type { SearchItem } from "@alter/protocol";

import { DAEMON_STATUS_CHANNEL } from "../shared/daemon";
import {
  ADDRESS_FOCUS_CHANNEL,
  NAV_CHANNELS,
  SHELL_OPEN_CHANNEL,
  COMMAND_BAR_CHANNELS,
  IMPORT_CHANNELS,
  PEEK_CHANNELS,
  SHELL_VIEW_CHANNEL,
  SPACE_CHANNELS,
  TASK_SPACE_CHANNELS,
  TAB_ACTIVATE_CHANNEL,
  TAB_CLOSE_CHANNEL,
  TAB_MOVE_CHANNEL,
  TAB_PIN_CHANNEL,
  TAB_SPLIT_CHANNEL,
  TAB_RESTORE_CHANNEL,
  TAB_UNSPLIT_CHANNEL,
  type AddressFocusRequest,
  type NavAction,
  type OpenRequest,
  type SpaceCreateRequest,
  type SpaceRequest,
  type TaskSpaceRequest,
  type CommandSearchRequest,
  type CommandSearchResults,
  type ImportProgressView,
  type ImportSources,
  type ImportStartRequest,
  type SpaceUpdateRequest,
  type TabMoveRequest,
  type TabPinRequest,
  type TabRequest,
  type ShellView,
} from "../shared/shell";
import { createActions } from "./actions";
import { alterHome, DaemonSupervisor } from "./daemon";
import { buildMenu } from "./menu";
import { Peek } from "./peek";
import { ShellLink } from "./shell-link";
import { guardStdio } from "./stdio";
import { TabViews } from "./tab-views";

// 最先执行：stdout / stderr 的读端关闭后，写日志与进程警告都不能变成未捕获异常
guardStdio();

// Chromium 的用户数据（各 Space 的 partition、缓存）放在 ALTER_HOME 下，必须在 ready 之前设置
app.setPath("userData", join(alterHome(), "chromium"));
// agent 经 alterd 的 CdpMux 直接对 Chromium 发 CDP：开 remote-debugging，只绑 127.0.0.1、端口随机。
// Chromium 把端口写进 userData/DevToolsActivePort，读到后经 Shell 的 cdpReady 告诉 alterd
app.commandLine.appendSwitch("remote-debugging-port", "0");
app.commandLine.appendSwitch("remote-debugging-address", "127.0.0.1");

function createMainWindow(): BrowserWindow {
  const win = new BrowserWindow({
    width: 1200,
    height: 800,
    title: "alter",
    webPreferences: {
      preload: join(__dirname, "../preload/index.js"),
      sandbox: true,
      contextIsolation: true,
    },
  });
  // electron-vite dev 时由环境变量给出 dev server 地址，构建后加载打包的页面
  const devUrl = process.env["ELECTRON_RENDERER_URL"];
  if (devUrl) {
    void win.loadURL(devUrl);
  } else {
    void win.loadFile(join(__dirname, "../renderer/index.html"));
  }
  return win;
}

function broadcast(channel: string, payload: unknown): void {
  for (const win of BrowserWindow.getAllWindows()) {
    win.webContents.send(channel, payload);
  }
}

let shellView: ShellView = { state: null, nav: { canGoBack: false, canGoForward: false, url: "" } };

function setShellView(next: ShellView): void {
  shellView = next;
  broadcast(SHELL_VIEW_CHANNEL, shellView);
}

let tabViews: TabViews | undefined;

/** 等 `shell.importSources` 的向导请求：requestId → 回调。 */
const importSourceWaiters = new Map<number, (sources: ImportSources) => void>();
let nextImportSourcesId = 1;
let peekRef: Peek | undefined;

const shell = new ShellLink(join(alterHome(), "shell.sock"), (cmd) => {
  if (cmd.type === "shell.stateChanged") {
    const state = {
      activeSpaceId: cmd.activeSpaceId,
      spaces: cmd.spaces,
      tabs: cmd.tabs,
      taskSpaces: cmd.taskSpaces,
      archived: cmd.archived,
    };
    setShellView({ ...shellView, state, error: undefined });
    tabViews?.apply(state);
    // 切了 Space 或 Peek 所在的 Space 被删除：关闭 Peek（在 apply 之后，恢复挂载用的是这份快照）
    peekRef?.sync(state);
  } else if (cmd.type === "shell.navigate") {
    tabViews?.navigate(cmd.tabId, cmd.url);
  } else if (cmd.type === "shell.discard") {
    tabViews?.discard(cmd.tabId);
  } else if (cmd.type === "shell.searchResults") {
    BrowserWindow.getAllWindows()[0]?.webContents.send(COMMAND_BAR_CHANNELS.results, {
      requestId: cmd.requestId,
      items: cmd.items,
    } satisfies CommandSearchResults);
  } else if (cmd.type === "shell.importSources") {
    importSourceWaiters.get(cmd.requestId)?.({ chrome: cmd.chrome, arc: cmd.arc });
    importSourceWaiters.delete(cmd.requestId);
  } else if (cmd.type === "shell.importProgress") {
    const progress: ImportProgressView = {
      requestId: cmd.requestId,
      importId: cmd.importId,
      status: cmd.status,
      item: cmd.item,
      counts: cmd.counts,
      warning: cmd.warning,
      error: cmd.error,
    };
    BrowserWindow.getAllWindows()[0]?.webContents.send(IMPORT_CHANNELS.progress, progress);
  } else if (cmd.type === "shell.error") {
    setShellView({ ...shellView, error: { code: cmd.code, message: cmd.message } });
  }
});

const actions = createActions({
  state: () => shellView.state,
  send: (event) => shell.send(event),
  nav: (action) => tabViews?.navAction(action),
  closing: (tabId) => tabViews?.closingTab(tabId),
  isClosing: (tabId) => tabViews?.isClosing(tabId) ?? false,
  openCommandBar: () => {
    const win = BrowserWindow.getAllWindows()[0];
    if (!win) return;
    // Peek 视图会盖住命令栏：先关掉
    peekRef?.close();
    // 命令栏期间内容区不显示页面：摘下视图，键盘焦点交给侧边栏所在的 webContents
    tabViews?.setSuppressed("commandBar", true);
    win.webContents.focus();
    win.webContents.send(COMMAND_BAR_CHANNELS.open, {});
  },
  focusAddress: (newTab) => {
    // 焦点可能在页面视图里：先把键盘焦点交还给侧边栏所在的 webContents
    const win = BrowserWindow.getAllWindows()[0];
    if (!win) return;
    win.webContents.focus();
    win.webContents.send(ADDRESS_FOCUS_CHANNEL, { newTab } satisfies AddressFocusRequest);
  },
});

const daemon = new DaemonSupervisor((status) => {
  broadcast(DAEMON_STATUS_CHANNEL, status);
  // alterd 就绪（含复用已在运行的实例）后连接 Shell Control Channel
  if (status.state === "running") {
    shell.connect();
    announceCdp();
  }
});

ipcMain.handle(DAEMON_STATUS_CHANNEL, () => daemon.current());
ipcMain.handle(SHELL_VIEW_CHANNEL, () => shellView);
ipcMain.on(TAB_ACTIVATE_CHANNEL, (_event, { tabId }: TabRequest) => actions.activateTab(tabId));
ipcMain.on(TAB_CLOSE_CHANNEL, (_event, { tabId }: TabRequest) => actions.closeTab(tabId));
ipcMain.on(TAB_RESTORE_CHANNEL, (_event, { tabId }: TabRequest) =>
  shell.send({ type: "shell.event.tabRestored", tabId }),
);
ipcMain.on(TAB_SPLIT_CHANNEL, (_event, { tabId }: TabRequest) => actions.splitWithActive(tabId));
ipcMain.on(TAB_UNSPLIT_CHANNEL, (_event, { tabId }: TabRequest) => actions.unsplit(tabId));
ipcMain.on(TAB_PIN_CHANNEL, (_event, request: TabPinRequest) => actions.pinTab(request));
ipcMain.on(TAB_MOVE_CHANNEL, (_event, request: TabMoveRequest) => actions.moveTab(request));
ipcMain.on(SPACE_CHANNELS.create, (_event, request: SpaceCreateRequest) => actions.createSpace(request));
ipcMain.on(SPACE_CHANNELS.update, (_event, request: SpaceUpdateRequest) => actions.updateSpace(request));
ipcMain.on(SPACE_CHANNELS.activate, (_event, { spaceId }: SpaceRequest) => actions.activateSpace(spaceId));
ipcMain.on(SPACE_CHANNELS.delete, (_event, { spaceId }: SpaceRequest) => actions.deleteSpace(spaceId));
ipcMain.on(TASK_SPACE_CHANNELS.return, (_event, { taskSpaceId }: TaskSpaceRequest) =>
  actions.returnTaskSpace(taskSpaceId),
);
ipcMain.on(TASK_SPACE_CHANNELS.stop, (_event, { taskSpaceId }: TaskSpaceRequest) => actions.stopTaskSpace(taskSpaceId));
for (const [action, channel] of Object.entries(NAV_CHANNELS) as [NavAction, string][]) {
  ipcMain.on(channel, () => actions.nav(action));
}
ipcMain.on(SHELL_OPEN_CHANNEL, (_event, request: OpenRequest) => actions.open(request));
ipcMain.on(PEEK_CHANNELS.close, () => peekRef?.close());
ipcMain.on(PEEK_CHANNELS.promote, () => peekRef?.promote());
ipcMain.on(COMMAND_BAR_CHANNELS.close, () => tabViews?.setSuppressed("commandBar", false));
ipcMain.on(COMMAND_BAR_CHANNELS.search, (_event, { requestId, query }: CommandSearchRequest) =>
  shell.send({ type: "shell.event.searchRequested", requestId, query }),
);
ipcMain.on(COMMAND_BAR_CHANNELS.execute, (_event, { item }: { item: SearchItem }) => {
  // 先恢复挂载，执行引起的快照再决定挂哪个页面
  tabViews?.setSuppressed("commandBar", false);
  actions.executeCommand(item);
});

ipcMain.handle(
  IMPORT_CHANNELS.sources,
  () =>
    new Promise<ImportSources>((resolve) => {
      const requestId = nextImportSourcesId++;
      importSourceWaiters.set(requestId, resolve);
      shell.send({ type: "shell.event.importSourcesRequested", requestId });
      // alterd 没有回应（未连接）时按空列表处理，向导不至于一直等
      setTimeout(() => {
        if (importSourceWaiters.delete(requestId)) resolve({ chrome: [], arc: [] });
      }, 5_000);
    }),
);
ipcMain.on(IMPORT_CHANNELS.start, (_event, request: ImportStartRequest) =>
  shell.send({ type: "shell.event.importRequested", ...request }),
);

app.whenReady().then(() => {
  Menu.setApplicationMenu(buildMenu(actions));
  const win = createMainWindow();
  const peek = new Peek(
    win,
    () => tabViews,
    (event) => shell.send(event),
  );
  peekRef = peek;
  tabViews = new TabViews(
    win,
    (event) => shell.send(event),
    (nav) => setShellView({ ...shellView, nav }),
    (url, tab) => peek.open(url, tab),
  );
  if (shellView.state) tabViews.apply(shellView.state);
  daemon.start();
});

// 退出前先关掉 alterd：拦下第一次退出，等 alterd 结束后再真正退出
let daemonStopped = false;
let stopping = false;
app.on("before-quit", (event) => {
  if (daemonStopped) return;
  event.preventDefault();
  if (stopping) return;
  stopping = true;
  void daemon.stop().finally(() => {
    daemonStopped = true;
    app.quit();
  });
});

// 骨架阶段：窗口全部关闭即退出，Dock 常驻行为留给后续需求
app.on("window-all-closed", () => {
  app.quit();
});

/** Chromium 的 remote-debugging 端口；读到之前为 undefined。 */
let cdpPort: number | undefined;

/** 端口与 Shell 连接都就绪时，告诉 alterd（连上或重连都要再发一次）。 */
function announceCdp(): void {
  if (cdpPort !== undefined) shell.send({ type: "shell.event.cdpReady", port: cdpPort });
}

/** 等 Chromium 写出 DevToolsActivePort（第一行是端口），最多 10 秒。 */
async function readCdpPort(): Promise<void> {
  const file = join(app.getPath("userData"), "DevToolsActivePort");
  for (let i = 0; i < 200; i++) {
    try {
      const port = Number(readFileSync(file, "utf8").split("\n")[0]);
      if (port > 0) {
        cdpPort = port;
        announceCdp();
        return;
      }
    } catch {
      // 还没写出
    }
    await new Promise((r) => setTimeout(r, 50));
  }
  console.error("[cdp] 10 秒内没有读到 DevToolsActivePort，agent 功能不可用");
}

void app.whenReady().then(readCdpPort);
