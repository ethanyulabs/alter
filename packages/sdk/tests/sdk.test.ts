// @alter/sdk 对假 Gateway 的契约测试：断言请求体与错误映射（N2 R20）
import { afterEach, expect, test } from "bun:test";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { AlterError, createAlter } from "../src/index";
import { fakeGateway } from "./fake-gateway";

const servers: { stop(): void }[] = [];
afterEach(() => {
  for (const s of servers.splice(0)) s.stop();
});

test("N2 R20.S1 taskSpace 按名称创建、按数字恢复，握手带 Bearer token", async () => {
  const gw = fakeGateway((req) => ({
    result: { id: req.method === "taskSpace.create" ? 3 : 7, spaceId: "S", ownership: "agent", pages: ["p1"] },
  }));
  servers.push(gw.server);
  const alter = createAlter({ gatewayFile: gw.gatewayFile });

  await alter.taskSpace("demo");
  await alter.taskSpace(7);
  alter.close();

  expect(gw.calls().map((r) => [r.method, r.params])).toEqual([
    ["taskSpace.create", { name: "demo" }],
    ["taskSpace.resume", { id: 7 }],
  ]);
  expect(gw.headers[0]).toBe(`Bearer ${"t".repeat(64)}`);
});

test("N2 R20.S2 错误映射为 AlterError 且不重试", async () => {
  const gw = fakeGateway((req) =>
    req.method === "page.click"
      ? { error: { code: "E_USER_CONTROL", message: "user has control" } }
      : { result: { id: 1, spaceId: "S", ownership: "agent", pages: ["p1"] } },
  );
  servers.push(gw.server);
  const alter = createAlter({ gatewayFile: gw.gatewayFile });
  const page = (await alter.taskSpace("demo")).page("p1");

  let caught: unknown;
  try {
    await page.click("text=Go");
  } catch (e) {
    caught = e;
  }
  alter.close();

  expect(caught).toBeInstanceOf(AlterError);
  expect((caught as AlterError).code).toBe("E_USER_CONTROL");
  expect(gw.calls().filter((r) => r.method === "page.click")).toHaveLength(1);
  expect(gw.calls().find((r) => r.method === "page.click")!.params).toEqual({ id: 1, label: "p1", selector: "text=Go" });
});

test("N2 R20.S3 连接失败：AlterError E_INTERNAL，提示打开 alter.app", async () => {
  const dir = mkdtempSync(join(tmpdir(), "alter-sdk-"));
  const gatewayFile = join(dir, "gateway.json");
  // 先占一个端口再释放，得到一个没有监听的端口
  const probe = Bun.serve({ port: 0, hostname: "127.0.0.1", fetch: () => new Response("") });
  const port = probe.port;
  probe.stop(true);
  writeFileSync(gatewayFile, JSON.stringify({ port, token: "x", cdpPort: 1, version: "0.1.0" }));
  const alter = createAlter({ gatewayFile });

  let caught: unknown;
  try {
    await alter.taskSpace("demo");
  } catch (e) {
    caught = e;
  }
  expect(caught).toBeInstanceOf(AlterError);
  expect((caught as AlterError).code).toBe("E_INTERNAL");
  expect((caught as AlterError).message).toContain("alter.app");
});

test("N2 CR1 page.mouse / keyboard / fetch 与 TaskSpace.cdp 的请求体", async () => {
  const gw = fakeGateway((req) =>
    req.method === "taskSpace.create"
      ? { result: { id: 3, name: "demo", spaceId: "S", ownership: "agent", pages: ["p1"] } }
      : req.method === "taskSpace.cdp"
        ? { result: { result: { product: "Fake" } } }
        : { result: { ok: true, durationMs: 1, popups: [] } },
  );
  servers.push(gw.server);
  const alter = createAlter({ gatewayFile: gw.gatewayFile });
  const t = await alter.taskSpace("demo");
  expect(t.name).toBe("demo");
  const p = t.page("p1");
  await p.mouse.click(1, 2);
  await p.keyboard.type("hi");
  await p.fetch("/data", { method: "POST" }, { saveAs: "/tmp/x" });
  expect(await t.cdp("Browser.getVersion")).toEqual({ product: "Fake" });
  const calls = gw.calls().slice(1);
  expect(calls.map((c) => [c.method, c.params])).toEqual([
    ["page.mouse", { id: 3, label: "p1", op: "click", args: { x: 1, y: 2 } }],
    ["page.keyboard", { id: 3, label: "p1", op: "type", args: { text: "hi" } }],
    ["page.fetch", { id: 3, label: "p1", url: "/data", options: { method: "POST" }, saveAs: "/tmp/x" }],
    ["taskSpace.cdp", { id: 3, method: "Browser.getVersion", params: {} }],
  ]);
});
