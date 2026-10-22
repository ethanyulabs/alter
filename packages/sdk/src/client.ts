// Gateway 客户端：读 gateway.json，带 Bearer token 连 ws://127.0.0.1:<port>/rpc，发 JSON-RPC 请求。
// 没有未完成的请求、也没有事件订阅时，空闲一会儿就断开（下次调用再连），脚本结束后进程才能退出。
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import type { ErrorCode, GatewayFile, RpcErrorData } from "@alter/protocol";

import { AlterError } from "./errors";

/** 与 alterd 约定的协议版本。 */
export const PROTOCOL_VERSION = 1;

const NOT_RUNNING = "无法连接 alterd，请先打开 alter.app";
const IDLE_CLOSE_MS = 20;

/** gateway.json 的位置：`ALTER_GATEWAY_FILE`，否则 `<ALTER_HOME>/gateway.json`。 */
export function defaultGatewayFile(): string {
  if (process.env["ALTER_GATEWAY_FILE"]) return process.env["ALTER_GATEWAY_FILE"];
  const home = process.env["ALTER_HOME"] ?? join(homedir(), "Library", "Application Support", "alter");
  return join(home, "gateway.json");
}

type Notification = { method: string; params: Record<string, unknown> };
type Pending = { resolve: (v: unknown) => void; reject: (e: unknown) => void };

export class Client {
  private ws: WebSocket | undefined;
  private opening: Promise<WebSocket> | undefined;
  private nextId = 1;
  private readonly pending = new Map<number, Pending>();
  private readonly listeners = new Set<(n: Notification) => void>();
  private idleTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(private readonly gatewayFile: string) {}

  /** 调用一个方法并返回 result；失败抛 AlterError，不重试。 */
  async call<T = unknown>(method: string, params: Record<string, unknown> = {}): Promise<T> {
    this.cancelIdle();
    const ws = await this.socket();
    const id = this.nextId++;
    const result = new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
    });
    ws.send(JSON.stringify({ jsonrpc: "2.0", id, method, params }));
    try {
      return await result;
    } finally {
      this.scheduleIdle();
    }
  }

  /** 订阅 Gateway 的通知；返回取消订阅的函数。订阅期间连接保持。 */
  subscribe(listener: (n: Notification) => void): () => void {
    this.cancelIdle();
    this.listeners.add(listener);
    void this.socket().catch(() => {});
    return () => {
      this.listeners.delete(listener);
      this.scheduleIdle();
    };
  }

  /** 立即断开。 */
  close(): void {
    this.cancelIdle();
    this.ws?.close();
    this.ws = undefined;
  }

  private socket(): Promise<WebSocket> {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) return Promise.resolve(this.ws);
    this.opening ??= this.open().finally(() => {
      this.opening = undefined;
    });
    return this.opening;
  }

  private async open(): Promise<WebSocket> {
    let gw: GatewayFile;
    try {
      gw = JSON.parse(readFileSync(this.gatewayFile, "utf8")) as GatewayFile;
    } catch {
      throw new AlterError("E_INTERNAL", `${NOT_RUNNING}（没有 ${this.gatewayFile}）`);
    }
    const ws = new WebSocket(`ws://127.0.0.1:${gw.port}/rpc`, {
      headers: { Authorization: `Bearer ${gw.token}` },
    } as unknown as string[]);
    await new Promise<void>((resolve, reject) => {
      ws.onopen = () => resolve();
      ws.onerror = () => reject(new AlterError("E_INTERNAL", `${NOT_RUNNING}（端口 ${gw.port}）`));
    });
    ws.onmessage = (ev) => this.receive(String(ev.data));
    ws.onclose = () => {
      if (this.ws === ws) this.ws = undefined;
      for (const [id, p] of this.pending) {
        p.reject(new AlterError("E_INTERNAL", "与 alterd 的连接已断开"));
        this.pending.delete(id);
      }
    };
    this.ws = ws;
    // 握手：核对协议版本
    const id = this.nextId++;
    const hello = new Promise<unknown>((resolve, reject) => this.pending.set(id, { resolve, reject }));
    ws.send(
      JSON.stringify({ jsonrpc: "2.0", id, method: "hello", params: { clientVersion: "0.1.0", protocolVersion: PROTOCOL_VERSION } }),
    );
    await hello;
    return ws;
  }

  private receive(text: string): void {
    let msg: { id?: number; result?: unknown; error?: { message: string; data?: RpcErrorData }; method?: string; params?: Record<string, unknown> };
    try {
      msg = JSON.parse(text);
    } catch {
      return;
    }
    if (msg.id === undefined) {
      if (msg.method) for (const l of this.listeners) l({ method: msg.method, params: msg.params ?? {} });
      return;
    }
    const p = this.pending.get(msg.id);
    if (!p) return;
    this.pending.delete(msg.id);
    if (msg.error) {
      const data = msg.error.data;
      const code: ErrorCode = data?.code ?? "E_INTERNAL";
      p.reject(new AlterError(code, data?.message ?? msg.error.message, data?.details as Record<string, unknown> | undefined));
    } else {
      p.resolve(msg.result);
    }
  }

  private scheduleIdle(): void {
    if (this.pending.size > 0 || this.listeners.size > 0) return;
    this.cancelIdle();
    this.idleTimer = setTimeout(() => this.close(), IDLE_CLOSE_MS);
  }

  private cancelIdle(): void {
    if (this.idleTimer !== undefined) clearTimeout(this.idleTimer);
    this.idleTimer = undefined;
  }
}
