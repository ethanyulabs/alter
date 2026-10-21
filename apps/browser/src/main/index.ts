// Electron 主进程入口：创建主窗口，托管 alterd
import { join } from "node:path";

import { app, BrowserWindow, ipcMain } from "electron";

import { DAEMON_STATUS_CHANNEL } from "../shared/daemon";
import { DaemonSupervisor } from "./daemon";

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

const daemon = new DaemonSupervisor((status) => {
  for (const win of BrowserWindow.getAllWindows()) {
    win.webContents.send(DAEMON_STATUS_CHANNEL, status);
  }
});

ipcMain.handle(DAEMON_STATUS_CHANNEL, () => daemon.current());

app.whenReady().then(() => {
  createMainWindow();
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
