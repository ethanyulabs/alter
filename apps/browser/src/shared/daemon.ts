// 主进程与渲染进程共用的 alterd 状态类型（IPC 通道 daemon.status 的载荷）

export type DaemonState = "starting" | "running" | "missing" | "exited";

export interface DaemonStatus {
  state: DaemonState;
  pid?: number;
  path?: string;
}

/** IPC 通道名：渲染进程 invoke 取当前状态，主进程 send 推送变化。 */
export const DAEMON_STATUS_CHANNEL = "daemon.status";

/** 渲染进程经 preload 拿到的 API。 */
export interface AlterApi {
  getDaemonStatus(): Promise<DaemonStatus>;
  onDaemonStatus(listener: (status: DaemonStatus) => void): () => void;
}
