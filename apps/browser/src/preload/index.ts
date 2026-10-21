// preload：经 contextBridge 给渲染进程暴露受限的 API
import { contextBridge, ipcRenderer, type IpcRendererEvent } from "electron";

import { DAEMON_STATUS_CHANNEL, type AlterApi, type DaemonStatus } from "../shared/daemon";
import {
  NAV_CHANNELS,
  SHELL_OPEN_CHANNEL,
  SHELL_VIEW_CHANNEL,
  TAB_ACTIVATE_CHANNEL,
  TAB_CLOSE_CHANNEL,
  type ShellApi,
  type ShellView,
} from "../shared/shell";

/** 订阅主进程推送的通道，返回取消订阅的函数。 */
function subscribe<T>(channel: string, listener: (payload: T) => void): () => void {
  const handler = (_event: IpcRendererEvent, payload: T) => listener(payload);
  ipcRenderer.on(channel, handler);
  return () => {
    ipcRenderer.off(channel, handler);
  };
}

const api: AlterApi & ShellApi = {
  getDaemonStatus: () => ipcRenderer.invoke(DAEMON_STATUS_CHANNEL),
  onDaemonStatus: (listener) => subscribe<DaemonStatus>(DAEMON_STATUS_CHANNEL, listener),
  getShellView: () => ipcRenderer.invoke(SHELL_VIEW_CHANNEL),
  onShellView: (listener) => subscribe<ShellView>(SHELL_VIEW_CHANNEL, listener),
  open: (request) => ipcRenderer.send(SHELL_OPEN_CHANNEL, request),
  activateTab: (tabId) => ipcRenderer.send(TAB_ACTIVATE_CHANNEL, { tabId }),
  closeTab: (tabId) => ipcRenderer.send(TAB_CLOSE_CHANNEL, { tabId }),
  navigate: (action) => ipcRenderer.send(NAV_CHANNELS[action], {}),
};

contextBridge.exposeInMainWorld("alter", api);
