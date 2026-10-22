// SDK 测试共用的假 Gateway（Bun.serve 的 WebSocket）
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

export interface Rpc {
  id: number;
  method: string;
  params: Record<string, unknown>;
}

/** 假 Gateway：记录收到的每个请求，`reply` 决定返回 result 还是 error；`push` 向所有连着的客户端推一条通知。 */
export function fakeGateway(reply: (req: Rpc) => { result?: unknown; error?: { code: string; message: string } }) {
  const received: Rpc[] = [];
  const sockets = new Set<{ send(data: string): unknown }>();
  const headers: (string | null)[] = [];
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    fetch(req, srv) {
      headers.push(req.headers.get("authorization"));
      if (srv.upgrade(req)) return undefined;
      return new Response("no", { status: 400 });
    },
    websocket: {
      open(ws) {
        sockets.add(ws);
      },
      close(ws) {
        sockets.delete(ws);
      },
      message(ws, data) {
        const req = JSON.parse(String(data)) as Rpc;
        received.push(req);
        const r = req.method === "hello" ? { result: { protocolVersion: 1, pageBudget: 8, spaces: [] } } : reply(req);
        ws.send(
          JSON.stringify(
            r.error
              ? { jsonrpc: "2.0", id: req.id, error: { code: -32000, message: r.error.message, data: r.error } }
              : { jsonrpc: "2.0", id: req.id, result: r.result },
          ),
        );
      },
    },
  });
  const dir = mkdtempSync(join(tmpdir(), "alter-sdk-"));
  const gatewayFile = join(dir, "gateway.json");
  writeFileSync(gatewayFile, JSON.stringify({ port: server.port, token: "t".repeat(64), cdpPort: 1, version: "0.1.0" }));
  const push = (method: string, params: Record<string, unknown>) => {
    for (const ws of sockets) ws.send(JSON.stringify({ jsonrpc: "2.0", method, params }));
  };
  return { server, received, headers, gatewayFile, push, calls: () => received.filter((r) => r.method !== "hello") };
}
