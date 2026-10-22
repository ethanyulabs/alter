// N2 agent 网关：经 `alter nodejs` 驱动真实的 app（Gateway → TaskSpace → CDP → Chromium）
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { runScript } from "./agent";
import { appLogs, dumpLogsOnFailure, launchApp, spawnApp, tempHome } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { contentUrls, now, openNewTab, ready, waitUntilAsync } from "./ui";

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

test("N2 R5.S3 侧边栏显示 agent 页面：多出一行带 p1 标记，Page A 仍激活，主进程有 about:blank 的页面", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    await openNewTab(win, server.input("/a"));
    await expect(win.getByTestId("tab-row")).toHaveText(["Page A"], { timeout: 2_000 });

    // 2 秒从发起创建算起（单调时钟；含启动 Bun 与脚本退出，只会高估）：脚本与界面检查同时进行，
    // 界面全部就绪、脚本（创建动作）结束的时刻都不能晚于截止时间
    const deadline = now() + 2_000;
    const left = () => Math.max(1, deadline - now());
    const script = runScript(home, `await taskSpace("demo")`);

    await expect(win.getByTestId("tab-row")).toHaveCount(2, { timeout: left() });
    await expect(win.getByTestId("agent-badge")).toHaveText("p1", { timeout: left() });
    await expect(win.getByTestId("tab-row").filter({ hasText: "Page A" })).toHaveClass(/active/);
    expect(await waitUntilAsync(left(), async () => (await contentUrls(app)).includes("about:blank"))).toBe(true);
    expect(now(), "界面没有在 2 秒内全部就绪").toBeLessThanOrEqual(deadline);
    const r = await script;
    expect(r.code, r.stderr).toBe(0);
    expect(now(), "创建动作没有在 2 秒内完成").toBeLessThanOrEqual(deadline);
  } finally {
    await app.close();
  }
});

test("N2 R11.S1 打开本地测试页：回执 ok、durationMs 为非负整数，标题为 Page A，退出码 0", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    await ready(await app.firstWindow());
    const r = await runScript(
      home,
      `const p = (await taskSpace("t")).page("p1");
       console.log(JSON.stringify(await p.goto(${JSON.stringify(server.url("/a"))})));
       console.log(await p.title());`,
    );
    expect(r.code, r.stderr).toBe(0);
    const [first, second] = r.stdout.trim().split("\n");
    const receipt = JSON.parse(first!);
    expect(receipt.ok).toBe(true);
    expect(Number.isInteger(receipt.durationMs) && receipt.durationMs >= 0).toBe(true);
    expect(second).toBe("Page A");
  } finally {
    await app.close();
  }
});

test("N2 R11.S2 导航超时：异常 code 为 E_TIMEOUT，脚本以退出码 1 结束", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    await ready(await app.firstWindow());
    const r = await runScript(
      home,
      `const p = (await taskSpace("t")).page("p1");
       try { await p.goto(${JSON.stringify(server.url("/delay"))}, { timeoutMs: 500 }); }
       catch (e) { console.log(e.code); process.exit(1); }`,
    );
    expect(r.stdout.trim(), r.stderr).toBe("E_TIMEOUT");
    expect(r.code).toBe(1);
  } finally {
    await app.close();
  }
});

/** 在新 app 里运行一段脚本（脚本前缀 `p` 为 p1 已打开 `path`）；返回结果。
 * `userTab` 为真时先开一个用户标签，p1 就是不挂在窗口上的离屏页面。 */
async function withPage(path: string, body: string, opts: { userTab?: boolean; env?: Record<string, string> } = {}) {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home, ...opts.env });
  try {
    const win = await app.firstWindow();
    await ready(win);
    if (opts.userTab) {
      await openNewTab(win, server.input("/a"));
      await expect(win.getByTestId("tab-row")).toHaveText(["Page A"], { timeout: 2_000 });
    }
    return await runScript(
      home,
      `const task = await taskSpace("t");
       const p = task.page("p1");
       await p.goto(${JSON.stringify(server.url(path))});
       ${body}`,
    );
  } finally {
    await app.close();
  }
}

test("N2 R12.S1 表单页快照：有 textbox Email 与 button Submit 两行，各带 @e 引用", async () => {
  const r = await withPage("/form", `console.log(await p.snapshot());`);
  expect(r.code, r.stderr).toBe(0);
  expect(r.stdout).toMatch(/- textbox "Email" \[@e\d+\]/);
  expect(r.stdout).toMatch(/- button "Submit" \[@e\d+\]/);
});

test("N2 R13.S1 填表并提交：正文含 submitted: a@b.c", async () => {
  const r = await withPage(
    "/form",
    `await p.fill('loc=role:textbox[name="Email"]', "a@b.c");
     await p.click("text=Submit");
     console.log(await p.evaluate(() => document.body.innerText));`,
  );
  expect(r.code, r.stderr).toBe(0);
  expect(r.stdout).toContain("submitted: a@b.c");
});

test("N2 R13.S2 用快照引用点击：正文含 submitted: x@y.z", async () => {
  const r = await withPage(
    "/form",
    `const snap = await p.snapshot();
     const ref = (role, name) => snap.split("\\n").find((l) => l.includes("- " + role + ' "' + name + '"')).match(/@e\\d+/)[0];
     await p.fill(ref("textbox", "Email"), "x@y.z");
     await p.click(ref("button", "Submit"));
     console.log(await p.evaluate(() => document.body.innerText));`,
  );
  expect(r.code, r.stderr).toBe(0);
  expect(r.stdout).toContain("submitted: x@y.z");
});

test("N2 R13.S3 选择器匹配多个元素：E_AMBIGUOUS_SELECTOR，details.count 为 2", async () => {
  const r = await withPage(
    "/two-buttons",
    `try { await p.click("text=Go"); } catch (e) { console.log(JSON.stringify({ code: e.code, details: e.details })); }`,
  );
  expect(r.code, r.stderr).toBe(0);
  expect(JSON.parse(r.stdout.trim())).toEqual({ code: "E_AMBIGUOUS_SELECTOR", details: { count: 2 } });
});

test("N2 R13.S4 选择器无匹配：E_NO_MATCH", async () => {
  const r = await withPage(
    "/form",
    `try { await p.click("text=Nope", { timeoutMs: 500 }); } catch (e) { console.log(e.code); }`,
  );
  expect(r.code, r.stderr).toBe(0);
  expect(r.stdout.trim()).toBe("E_NO_MATCH");
});

test("N2 R19.S1 回执日志：page.goto 成功一行、page.click 失败一行（E_NO_MATCH）", async () => {
  const r = await withPage(
    "/a",
    `try { await p.click("text=Nope", { timeoutMs: 300 }); } catch (e) { console.log(e.code); }`,
  );
  expect(r.code, r.stderr).toBe(0);
  const receipts = appLogs()
    .filter((l) => l.includes('"receipt"'))
    .map((l) => JSON.parse(l.slice(l.indexOf("{"))) as { fields: Record<string, unknown> })
    .map((j) => j.fields);
  expect(receipts.some((f) => f["method"] === "page.goto" && f["ok"] === true)).toBe(true);
  expect(receipts.some((f) => f["method"] === "page.click" && f["ok"] === false && f["error_code"] === "E_NO_MATCH")).toBe(
    true,
  );
});

test("N2 R14.S1 先处理对话框再操作：E_DIALOG_OPEN，details.message 为 hi；accept 后点击成功", async () => {
  // 不经 Playwright 启动：它会自动关掉对话框
  const home = tempHome();
  const app = await spawnApp(home);
  const r = await runScript(
    home,
    `const p = (await taskSpace("t")).page("p1");
     await p.goto(${JSON.stringify(server.url("/alert"))});
     try { await p.click("body"); console.log("no-error"); }
     catch (e) { console.log(JSON.stringify({ code: e.code, message: e.details?.message })); }
     await p.dialog({ accept: true });
     console.log(JSON.stringify(await p.click("body")));`,
  ).finally(() => app.close());
  expect(r.code, r.stderr).toBe(0);
  const [first, second] = r.stdout.trim().split("\n");
  expect(JSON.parse(first!)).toEqual({ code: "E_DIALOG_OPEN", message: "hi" });
  expect(JSON.parse(second!).ok).toBe(true);
});

test("N2 R15.S1 截图（p1 是离屏页面）：width、height 大于 0，文件以 PNG 签名开头", async () => {
  const dir = mkdtempSync(join(tmpdir(), "alter-shot-"));
  const path = join(dir, "a.png");
  const r = await withPage("/a", `console.log(JSON.stringify(await p.screenshot({ path: ${JSON.stringify(path)} })));`, {
    userTab: true,
  });
  expect(r.code, r.stderr).toBe(0);
  const shot = JSON.parse(r.stdout.trim());
  expect(shot.width).toBeGreaterThan(0);
  expect(shot.height).toBeGreaterThan(0);
  expect([...readFileSync(path).subarray(0, 4)]).toEqual([0x89, 0x50, 0x4e, 0x47]);
});

test("N2 R15.S2 截图到相对路径被拒绝：E_INVALID_ARGUMENT", async () => {
  const r = await withPage("/a", `try { await p.screenshot({ path: "a.png" }); } catch (e) { console.log(e.code); }`);
  expect(r.code, r.stderr).toBe(0);
  expect(r.stdout.trim()).toBe("E_INVALID_ARGUMENT");
});

test("N2 R15.S3 evaluate 返回值：输出 42", async () => {
  const r = await withPage("/a", `console.log(await p.evaluate((x) => x + 1, 41));`);
  expect(r.code, r.stderr).toBe(0);
  expect(r.stdout.trim()).toBe("42");
});

test("N2 R15.S4 等待选择器超时：E_TIMEOUT", async () => {
  const r = await withPage(
    "/a",
    `try { await p.waitFor({ selector: "#never" }, { timeoutMs: 300 }); } catch (e) { console.log(e.code); }`,
  );
  expect(r.code, r.stderr).toBe(0);
  expect(r.stdout.trim()).toBe("E_TIMEOUT");
});

test("N2 R16.S1 点击新窗口链接：回执 popups 为 [{label:p2,…}]，p2 的 url 为 /b 的地址", async () => {
  const r = await withPage(
    "/links",
    `console.log(JSON.stringify((await p.click("text=Open B")).popups));
     console.log(await task.page("p2").url());`,
  );
  expect(r.code, r.stderr).toBe(0);
  const [popups, url] = r.stdout.trim().split("\n");
  expect(JSON.parse(popups!)).toEqual([{ label: "p2", targetId: expect.any(String) }]);
  expect(url).toBe(server.url("/b"));
});

test("N2 R16.S2 下载文件：输出 D/report.txt，文件存在", async () => {
  const dir = mkdtempSync(join(tmpdir(), "alter-dl-"));
  const r = await withPage(
    "/a",
    `const d = p.waitForEvent("download");
     await p.goto(${JSON.stringify(server.url("/download"))});
     console.log((await d).path);`,
    { env: { ALTER_DOWNLOAD_DIR: dir } },
  );
  expect(r.code, r.stderr).toBe(0);
  expect(r.stdout.trim()).toBe(join(dir, "report.txt"));
  expect(existsSync(join(dir, "report.txt"))).toBe(true);
});

test("N2 R17.S1 透传 Runtime.evaluate：result.value 为 2", async () => {
  const r = await withPage(
    "/a",
    `console.log(JSON.stringify(await p.cdp("Runtime.evaluate", { expression: "1+1", returnByValue: true })));`,
  );
  expect(r.code, r.stderr).toBe(0);
  expect(JSON.parse(r.stdout.trim()).result.value).toBe(2);
});

test("N2 CR3 waitFor 的函数是异步的：返回 false 时超时，返回 true 时满足", async () => {
  const r = await withPage(
    "/a",
    `try { await p.waitFor({ fn: async () => false }, { timeoutMs: 300 }); console.log("matched"); }
     catch (e) { console.log(e.code); }
     console.log(await p.waitFor({ fn: async () => document.title === "Page A" }, { timeoutMs: 1000 }));`,
  );
  expect(r.code, r.stderr).toBe(0);
  expect(r.stdout.trim().split("\n")).toEqual(["E_TIMEOUT", "true"]);
});

test("N2 CR5 每个 page.* 调用都有回执行：page.close 也写，成功时 error_code 为空字符串", async () => {
  const r = await withPage("/a", `await p.close();`);
  expect(r.code, r.stderr).toBe(0);
  const receipts = appLogs()
    .filter((l) => l.includes('"receipt"'))
    .map((l) => (JSON.parse(l.slice(l.indexOf("{"))) as { fields: Record<string, unknown> }).fields);
  for (const method of ["page.goto", "page.close"]) {
    expect(receipts.find((f) => f["method"] === method)).toMatchObject({ ok: true, error_code: "" });
  }
});

test("N2 CR6 waitFor 的函数返回永不完成的 Promise：timeoutMs 后 E_TIMEOUT，之后的命令照常执行", async () => {
  const r = await withPage(
    "/a",
    `const t0 = Date.now();
     try { await p.waitFor({ fn: () => new Promise(() => {}) }, { timeoutMs: 300 }); console.log("matched"); }
     catch (e) { console.log(e.code, Date.now() - t0 < 2000); }
     console.log(await p.title());`,
  );
  expect(r.code, r.stderr).toBe(0);
  expect(r.stdout.trim().split("\n")).toEqual(["E_TIMEOUT true", "Page A"]);
});

test("N2 V2 子树快照：scope subtree + root 只含该元素之下的节点；viewport 被拒绝", async () => {
  const r = await withPage(
    "/form",
    `const s = await p.snapshot({ scope: "subtree", root: "form" });
     console.log(JSON.stringify({ root: s.includes("RootWebArea"), email: s.includes('- textbox "Email" [@e') }));
     try { await p.snapshot({ scope: "viewport" }); console.log("ok"); } catch (e) { console.log(e.code); }`,
  );
  expect(r.code, r.stderr).toBe(0);
  const [first, second] = r.stdout.trim().split("\n");
  expect(JSON.parse(first!)).toEqual({ root: false, email: true });
  expect(second).toBe("E_INVALID_ARGUMENT");
});

test("N2 V3 元素动作的 actionLabel 写进回执日志", async () => {
  const r = await withPage("/form", `await p.click("text=Submit", { actionLabel: "提交表单" });`);
  expect(r.code, r.stderr).toBe(0);
  const click = appLogs()
    .filter((l) => l.includes('"receipt"'))
    .map((l) => (JSON.parse(l.slice(l.indexOf("{"))) as { fields: Record<string, unknown> }).fields)
    .find((f) => f["method"] === "page.click");
  expect(click).toMatchObject({ ok: true, action_label: "提交表单" });
});

test("N2 V5 reload 按 waitUntil 等待：domcontentloaded 不等图片，load 等图片超时", async () => {
  const r = await withPage(
    "/a",
    `await p.goto(${JSON.stringify(server.url("/slow-image"))}, { waitUntil: "domcontentloaded" });
     console.log((await p.reload({ waitUntil: "domcontentloaded", timeoutMs: 2000 })).ok);
     try { await p.reload({ timeoutMs: 1000 }); console.log("loaded"); } catch (e) { console.log(e.code); }`,
  );
  expect(r.code, r.stderr).toBe(0);
  expect(r.stdout.trim().split("\n")).toEqual(["true", "E_TIMEOUT"]);
});
