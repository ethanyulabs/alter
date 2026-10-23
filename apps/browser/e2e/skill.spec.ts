// RC N3 R7：SKILL.md 第一个 bash 代码块里的快速上手示例，在 alter 运行时原样执行成功
// （只把示例里的 https://example.com 换成本地测试服务器的页面）
import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { expect, test } from "@playwright/test";

import { dumpLogsOnFailure, launchApp, tempHome, waitUntil } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { ready } from "./ui";

const REPO = resolve(__dirname, "../../..");

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

test("RC N3 R7.S1 对本地页面跑 SKILL.md 的快速上手示例：退出码 0，输出 Example Domain", async () => {
  const skill = readFileSync(join(REPO, "skills/alter-browser/SKILL.md"), "utf8");
  const block = skill.match(/```bash\n([\s\S]*?)```/)?.[1];
  expect(block, "SKILL.md 里应当有 bash 代码块").toBeTruthy();
  expect(block).toContain("https://example.com");
  const script = block!.replaceAll("https://example.com", server.url("/example"));

  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await ready(win);
    expect(await waitUntil(10_000, () => existsSync(join(home, "gateway.json"))), "应当出现 gateway.json").toBe(true);
    const env: Record<string, string> = {
      ...(process.env as Record<string, string>),
      ALTER_HOME: home,
      PATH: `${join(REPO, "target/debug")}:${process.env["PATH"] ?? ""}`,
    };
    // FORCE_COLOR 由 Playwright 设置，会让 Bun 给输出加颜色转义码
    delete env["FORCE_COLOR"];
    // 异步启动：本地测试服务器跑在本进程里，同步等待会让它无法响应脚本里的 goto
    const run = await new Promise<{ status: number | null; stdout: string; stderr: string }>((done) => {
      const child = spawn("bash", ["-c", script], { env });
      let stdout = "";
      let stderr = "";
      child.stdout.on("data", (c: Buffer) => (stdout += c.toString()));
      child.stderr.on("data", (c: Buffer) => (stderr += c.toString()));
      const timer = setTimeout(() => child.kill("SIGKILL"), 20_000);
      child.on("close", (status) => {
        clearTimeout(timer);
        done({ status, stdout, stderr });
      });
    });
    expect(run.status, `stdout：${run.stdout}\nstderr：${run.stderr}`).toBe(0);
    expect(run.stdout).toContain("Example Domain");
  } finally {
    await app.close();
  }
});
