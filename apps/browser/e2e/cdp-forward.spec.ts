// N2 CDP 转发端点：Playwright 经 alterd 的 cdpPort 连接 app 里的 Chromium
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { chromium, expect, test } from "@playwright/test";

import { dumpLogsOnFailure, launchApp, tempHome } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { openNewTab, ready } from "./ui";

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

function gateway(home: string): { cdpPort: number; token: string } {
  return JSON.parse(readFileSync(join(home, "gateway.json"), "utf8"));
}

test("N2 R18.S1 Playwright 连接：connectOverCDP 成功，某个页面的 url 为 /a 的地址", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/a"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Page A"], { timeout: 2_000 });
    const { cdpPort, token } = gateway(home);
    // WebSocket 也要经过转发端点，而不是被指到 Chromium 自己的端口
    const version = await fetch(`http://127.0.0.1:${cdpPort}/json/version`, {
      headers: { Authorization: `Bearer ${token}` },
    }).then((r) => r.json() as Promise<{ webSocketDebuggerUrl: string }>);
    expect(new URL(version.webSocketDebuggerUrl).port).toBe(String(cdpPort));
    const browser = await chromium.connectOverCDP(`http://127.0.0.1:${cdpPort}`, {
      headers: { Authorization: `Bearer ${token}` },
      timeout: 5_000,
    });
    try {
      const urls = browser.contexts().flatMap((c) => c.pages().map((p) => p.url()));
      expect(urls).toContain(server.url("/a"));
    } finally {
      await browser.close();
    }
  } finally {
    await app.close();
  }
});

test("N2 R18.S2 缺少 token：/json/version 返回 401", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    await ready(await app.firstWindow());
    const { cdpPort } = gateway(home);
    const res = await fetch(`http://127.0.0.1:${cdpPort}/json/version`);
    expect(res.status).toBe(401);
  } finally {
    await app.close();
  }
});
