// SDK 的接管与交还：handOff、waitForControl、takeOverTaskSpace，对假 Gateway（N3 R7）
import { afterEach, expect, test } from "bun:test";

import { AlterError, createAlter } from "../src/index";
import { fakeGateway, type Rpc } from "./fake-gateway";

const servers: { stop(): void }[] = [];
afterEach(() => {
  for (const s of servers.splice(0)) s.stop();
});

const info = (ownership: string) => ({ id: 3, name: "login", spaceId: "S", ownership, pages: ["p1"] });

/** 创建、恢复都返回 ownership 为 user 的 TaskSpace 3。 */
function userControlled(req: Rpc) {
  return req.method === "taskSpace.create" || req.method === "taskSpace.resume" ? { result: info("user") } : { result: {} };
}

test("N3 R7.S1 收到交还后返回", async () => {
  const gw = fakeGateway(userControlled);
  servers.push(gw.server);
  const t = await createAlter({ gatewayFile: gw.gatewayFile }).taskSpace("login");
  const started = performance.now();
  const waiting = t.waitForControl({ timeoutMs: 2000 });
  setTimeout(() => gw.push("taskSpace.ownershipChanged", { id: 3, ownership: "agent", reason: "returned" }), 200);
  await waiting;
  expect(performance.now() - started).toBeLessThan(1000);
  expect(t.ownership).toBe("agent");
});

test("N3 R7.S2 超时：AlterError E_TIMEOUT", async () => {
  const gw = fakeGateway(userControlled);
  servers.push(gw.server);
  const t = await createAlter({ gatewayFile: gw.gatewayFile }).taskSpace("login");
  const e = await t.waitForControl({ timeoutMs: 300 }).catch((err: unknown) => err);
  expect(e).toBeInstanceOf(AlterError);
  expect((e as AlterError).code).toBe("E_TIMEOUT");
});

test("N3 R7.S3 E_USER_CONTROL 不被重试：page.goto 恰好请求一次", async () => {
  const gw = fakeGateway((req) =>
    req.method === "page.goto"
      ? { error: { code: "E_USER_CONTROL", message: "用户控制中" } }
      : { result: info("agent") },
  );
  servers.push(gw.server);
  const t = await createAlter({ gatewayFile: gw.gatewayFile }).taskSpace("login");
  const e = await t
    .page("p1")
    .goto("https://example.com")
    .catch((err: unknown) => err);
  expect(e).toBeInstanceOf(AlterError);
  expect((e as AlterError).code).toBe("E_USER_CONTROL");
  expect(gw.calls().filter((c) => c.method === "page.goto")).toHaveLength(1);
});

test("N3 takeOverTaskSpace 返回的 TaskSpace 带 userPage", async () => {
  const gw = fakeGateway((req) =>
    req.method === "taskSpace.takeOver"
      ? { result: { ownership: "agent", userPage: "p2" } }
      : { result: { ...info("agent"), pages: ["p1", "p2"] } },
  );
  servers.push(gw.server);
  const t = await createAlter({ gatewayFile: gw.gatewayFile }).takeOverTaskSpace(3);
  expect(t.ownership).toBe("agent");
  expect(t.userPage()?.label).toBe("p2");
});

test("N3 waitForControl：等之前用户已经交还时立即返回，不等通知", async () => {
  const gw = fakeGateway((req) =>
    req.method === "taskSpace.create" ? { result: info("user") } : { result: info("agent") },
  );
  servers.push(gw.server);
  const t = await createAlter({ gatewayFile: gw.gatewayFile }).taskSpace("login");
  await t.waitForControl({ timeoutMs: 300 });
  expect(t.ownership).toBe("agent");
});
