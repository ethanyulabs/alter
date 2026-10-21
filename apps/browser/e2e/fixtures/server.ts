// e2e 用的本地 HTTP 服务器：只监听 127.0.0.1，提供带标题的测试页并记录每个路径被请求的次数
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";

/** 路径到页面标题：`/a` → `Page A`，`/n/7` → `Page 7`，`/count` → `Count`。 */
function titleOf(path: string): string | undefined {
  const letter = /^\/([a-z])$/.exec(path);
  if (letter) return `Page ${letter[1]!.toUpperCase()}`;
  const n = /^\/n\/(\d+)$/.exec(path);
  if (n) return `Page ${n[1]}`;
  if (path === "/count") return "Count";
  return undefined;
}

/** 1×1 透明 PNG，作为测试页的 favicon。 */
const PNG_1X1 = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==",
  "base64",
);

/** 固定内容的资源：iframe 页内导航（CR1）、favicon（CR6）、新窗口链接与 blob 弹窗（R14）。 */
const SPECIAL: Record<string, { type: string; body: string | Buffer }> = {
  "/favicon.png": { type: "image/png", body: PNG_1X1 },
  "/blob": {
    type: "text/html; charset=utf-8",
    body: `<!doctype html><title>Blob Opener</title><p>blob</p>`,
  },
  "/popup": {
    type: "text/html; charset=utf-8",
    body: `<!doctype html><title>Popup</title><a id="pop" target="_blank" href="/b">pop</a>`,
  },
  "/frame": {
    type: "text/html; charset=utf-8",
    body: `<!doctype html><title>Frame</title><iframe src="/inner"></iframe>`,
  },
  "/inner": {
    type: "text/html; charset=utf-8",
    body: `<!doctype html><title>Inner</title><script>location.hash = "x";</script>`,
  },
};

export interface TestServer {
  port: number;
  /** 地址框里输入的形式（不带协议）：`127.0.0.1:<port><path>` */
  input(path: string): string;
  /** 浏览器里看到的完整 URL：`http://127.0.0.1:<port><path>` */
  url(path: string): string;
  /** 该路径被请求的次数 */
  hits(path: string): number;
  close(): Promise<void>;
}

export async function startServer(): Promise<TestServer> {
  const hits = new Map<string, number>();
  const server: Server = createServer((req, res) => {
    const path = new URL(req.url ?? "/", "http://127.0.0.1").pathname;
    hits.set(path, (hits.get(path) ?? 0) + 1);
    const special = SPECIAL[path];
    if (special) {
      res.writeHead(200, { "content-type": special.type, "cache-control": "no-store" });
      res.end(special.body);
      return;
    }
    const title = titleOf(path);
    if (title === undefined) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" });
    res.end(`<!doctype html><title>${title}</title><link rel="icon" href="/favicon.png"><p>${title}</p>`);
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = (server.address() as AddressInfo).port;
  return {
    port,
    input: (path) => `127.0.0.1:${port}${path}`,
    url: (path) => `http://127.0.0.1:${port}${path}`,
    hits: (path) => hits.get(path) ?? 0,
    close: () => new Promise((resolve) => server.close(() => resolve())),
  };
}
