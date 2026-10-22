// Electron 主进程入口：创建主窗口，托管 alterd
import { join } from "node:path";

import { app, BrowserWindow, ipcMain } from "electron";

import { DAEMON_STATUS_CHANNEL } from "../shared/daemon";
import {
  NAV_CHANNELS,
  SHELL_OPEN_CHANNEL,
  SHELL_VIEW_CHANNEL,
  TAB_ACTIVATE_CHANNEL,
  TAB_CLOSE_CHANNEL,
  type NavAction,
  type OpenRequest,
  type TabRequest,
  type ShellView,
} from "../shared/shell";
import { alterHome, DaemonSupervisor } from "./daemon";
import { ShellLink } from "./shell-link";
import { guardStdio } from "./stdio";
import { TabViews } from "./tab-views";

// 最先执行：stdout / stderr 的读端关闭后，写日志与进程警告都不能变成未捕获异常
guardStdio();

// Chromium 的用户数据（各 Space 的 partition、缓存）放在 ALTER_HOME 下，必须在 ready 之前设置
app.setPath("userData", join(alterHome(), "chromium"));

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

const shell = new ShellLink(join(alterHome(), "shell.sock"), (cmd) => {
  if (cmd.type === "shell.stateChanged") {
    const state = { activeSpaceId: cmd.activeSpaceId, spaces: cmd.spaces, tabs: cmd.tabs };
    setShellView({ ...shellView, state, error: undefined });
    tabViews?.apply(state);
  } else if (cmd.type === "shell.navigate") {
    tabViews?.navigate(cmd.tabId, cmd.url);
  } else if (cmd.type === "shell.error") {
    setShellView({ ...shellView, error: { code: cmd.code, message: cmd.message } });
  }
});

const daemon = new DaemonSupervisor((status) => {
  broadcast(DAEMON_STATUS_CHANNEL, status);
  // alterd 就绪（含复用已在运行的实例）后连接 Shell Control Channel
  if (status.state === "running") shell.connect();
});

ipcMain.handle(DAEMON_STATUS_CHANNEL, () => daemon.current());
ipcMain.handle(SHELL_VIEW_CHANNEL, () => shellView);
ipcMain.on(TAB_ACTIVATE_CHANNEL, (_event, { tabId }: TabRequest) => {
  shell.send({ type: "shell.event.tabActivated", tabId });
});
ipcMain.on(TAB_CLOSE_CHANNEL, (_event, { tabId }: TabRequest) => {
  shell.send({ type: "shell.event.tabClosed", tabId });
});
for (const [action, channel] of Object.entries(NAV_CHANNELS) as [NavAction, string][]) {
  ipcMain.on(channel, () => tabViews?.navAction(action));
}
ipcMain.on(SHELL_OPEN_CHANNEL, (_event, request: OpenRequest) => {
  const state = shellView.state;
  if (!state) return;
  const spaceId = state.activeSpaceId;
  // 不是新标签模式且当前 Space 有激活标签时在该标签打开，否则新建标签
  const active = state.tabs.find((t) => t.spaceId === spaceId && t.active);
  const tabId = request.newTab ? undefined : active?.id;
  shell.send({ type: "shell.event.openRequested", spaceId, input: request.input, ...(tabId ? { tabId } : {}) });
});

app.whenReady().then(() => {
  const win = createMainWindow();
  tabViews = new TabViews(
    win,
    (event) => shell.send(event),
    (nav) => setShellView({ ...shellView, nav }),
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
