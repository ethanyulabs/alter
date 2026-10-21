// Shell Control Channel 的 Electron 端：连接 alterd 的 shell.sock，按行收发 JSON
import { createConnection, type Socket } from "node:net";

import type { ShellCommand, ShellEvent } from "@alter/protocol";

export class ShellLink {
  private socket: Socket | undefined;
  private buffer = "";

  constructor(
    private readonly path: string,
    private readonly onCommand: (cmd: ShellCommand) => void,
  ) {}

  /** 连接 alterd；已连接时什么也不做。断开后不自动重连（alterd 退出时页面显示「已退出」）。 */
  connect(): void {
    if (this.socket) return;
    const socket = createConnection(this.path);
    socket.setEncoding("utf8");
    socket.on("data", (chunk: string) => this.receive(chunk));
    socket.on("error", (e) => console.error(`[shell] ${e.message}`));
    socket.on("close", () => {
      if (this.socket === socket) this.socket = undefined;
    });
    this.socket = socket;
  }

  send(event: ShellEvent): void {
    this.socket?.write(`${JSON.stringify(event)}\n`);
  }

  private receive(chunk: string): void {
    this.buffer += chunk;
    let nl: number;
    while ((nl = this.buffer.indexOf("\n")) >= 0) {
      const line = this.buffer.slice(0, nl);
      this.buffer = this.buffer.slice(nl + 1);
      try {
        this.onCommand(JSON.parse(line) as ShellCommand);
      } catch {
        console.error(`[shell] 无法解析的消息：${line.slice(0, 200)}`);
      }
    }
  }
}
