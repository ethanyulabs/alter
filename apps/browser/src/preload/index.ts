// preload：经 contextBridge 给渲染进程暴露受限的 API
import { contextBridge, ipcRenderer, type IpcRendererEvent } from "electron";

import { DAEMON_STATUS_CHANNEL, type AlterApi, type DaemonStatus } from "../shared/daemon";
import {
  ADDRESS_FOCUS_CHANNEL,
  COMMAND_BAR_CHANNELS,
  IMPORT_CHANNELS,
  PEEK_CHANNELS,
  NAV_CHANNELS,
  SHELL_OPEN_CHANNEL,
  SHELL_VIEW_CHANNEL,
  SPACE_CHANNELS,
  TAB_ACTIVATE_CHANNEL,
  TAB_CLOSE_CHANNEL,
  TAB_MOVE_CHANNEL,
  TAB_PIN_CHANNEL,
  TAB_SPLIT_CHANNEL,
  TAB_RESTORE_CHANNEL,
  TAB_UNSPLIT_CHANNEL,
  TASK_SPACE_CHANNELS,
  type AddressFocusRequest,
  type CommandSearchResults,
  type ImportProgressView,
  type PeekState,
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
  pinTab: (request) => ipcRenderer.send(TAB_PIN_CHANNEL, request),
  splitTab: (tabId) => ipcRenderer.send(TAB_SPLIT_CHANNEL, { tabId }),
  restoreTab: (tabId) => ipcRenderer.send(TAB_RESTORE_CHANNEL, { tabId }),
  unsplitTab: (tabId) => ipcRenderer.send(TAB_UNSPLIT_CHANNEL, { tabId }),
  moveTab: (request) => ipcRenderer.send(TAB_MOVE_CHANNEL, request),
  navigate: (action) => ipcRenderer.send(NAV_CHANNELS[action], {}),
  createSpace: (request) => ipcRenderer.send(SPACE_CHANNELS.create, request),
  updateSpace: (request) => ipcRenderer.send(SPACE_CHANNELS.update, request),
  activateSpace: (spaceId) => ipcRenderer.send(SPACE_CHANNELS.activate, { spaceId }),
  deleteSpace: (spaceId) => ipcRenderer.send(SPACE_CHANNELS.delete, { spaceId }),
  returnTaskSpace: (taskSpaceId) => ipcRenderer.send(TASK_SPACE_CHANNELS.return, { taskSpaceId }),
  stopTaskSpace: (taskSpaceId) => ipcRenderer.send(TASK_SPACE_CHANNELS.stop, { taskSpaceId }),
  onPeekState: (listener) => subscribe<PeekState>(PEEK_CHANNELS.state, listener),
  closePeek: () => ipcRenderer.send(PEEK_CHANNELS.close, {}),
  promotePeek: () => ipcRenderer.send(PEEK_CHANNELS.promote, {}),
  onCommandBarOpen: (listener) => subscribe<Record<string, never>>(COMMAND_BAR_CHANNELS.open, () => listener()),
  closeCommandBar: () => ipcRenderer.send(COMMAND_BAR_CHANNELS.close, {}),
  search: (request) => ipcRenderer.send(COMMAND_BAR_CHANNELS.search, request),
  onSearchResults: (listener) => subscribe<CommandSearchResults>(COMMAND_BAR_CHANNELS.results, listener),
  executeCommand: (item) => ipcRenderer.send(COMMAND_BAR_CHANNELS.execute, { item }),
  onAddressFocus: (listener) => subscribe<AddressFocusRequest>(ADDRESS_FOCUS_CHANNEL, listener),
  importSources: () => ipcRenderer.invoke(IMPORT_CHANNELS.sources),
  startImport: (request) => ipcRenderer.send(IMPORT_CHANNELS.start, request),
  onImportProgress: (listener) => subscribe<ImportProgressView>(IMPORT_CHANNELS.progress, listener),
};

contextBridge.exposeInMainWorld("alter", api);
