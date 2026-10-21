// preload：经 contextBridge 给渲染进程暴露受限的 API
import { contextBridge, ipcRenderer, type IpcRendererEvent } from "electron";

import { DAEMON_STATUS_CHANNEL, type AlterApi, type DaemonStatus } from "../shared/daemon";

const api: AlterApi = {
  getDaemonStatus: () => ipcRenderer.invoke(DAEMON_STATUS_CHANNEL),
  onDaemonStatus: (listener) => {
    const handler = (_event: IpcRendererEvent, status: DaemonStatus) => listener(status);
    ipcRenderer.on(DAEMON_STATUS_CHANNEL, handler);
    return () => {
      ipcRenderer.off(DAEMON_STATUS_CHANNEL, handler);
    };
  },
};

contextBridge.exposeInMainWorld("alter", api);
