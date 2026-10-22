// 启动打包前 app 的公共函数
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { _electron as electron, type ElectronApplication, type TestInfo } from "@playwright/test";

/** electron-vite 构建出的主进程入口。 */
export const MAIN_ENTRY = resolve(__dirname, "../out/main/index.js");

/** 新建一个空的临时数据目录。 */
export function tempHome(): string {
  return mkdtempSync(join(tmpdir(), "alter-e2e-"));
}

/**
 * 启动 app。未指定 `ALTER_HOME` 时用一个新的临时目录，测试绝不碰真实数据目录。
 * `ALTER_E2E_ARGS` 可追加 Electron 命令行参数（例如 --no-sandbox）。
 */
export async function launchApp(env: Record<string, string> = {}): Promise<ElectronApplication> {
  const extra = process.env.ALTER_E2E_ARGS?.split(" ").filter(Boolean) ?? [];
  const app = await electron.launch({
    args: [MAIN_ENTRY, ...extra],
    env: { ...process.env, ALTER_HOME: tempHome(), ...env } as Record<string, string>,
  });
  // 单个动作（点击、输入）最多等 5 秒：元素缺失时报出具体的定位器，而不是等满整个用例的超时。
  // 配置文件里的 use.actionTimeout 管不到 Electron 窗口，只能在这里设
  app.context().setDefaultTimeout(5_000);
  // 收集主进程（含转发的 alterd stderr）的输出，用例失败时由 dumpLogsOnFailure 打印
  const pid = app.process().pid;
  for (const stream of [app.process().stdout, app.process().stderr]) {
    stream?.on("data", (chunk: Buffer) => logs.push(`[pid ${pid}] ${chunk.toString().trimEnd()}`));
  }
  app.process().on("exit", (code, signal) => logs.push(`[pid ${pid}] exited code=${code} signal=${signal}`));
  return app;
}

const logs: string[] = [];

/**
 * 不经 Playwright 直接启动 app，等到 `gateway.json` 写出。Playwright 启动的 app 会被它附着所有页面，
 * 并自动关掉没人监听的 JS 对话框；只经 `alter` CLI 驱动、要看到对话框的用例用这个。
 */
export async function spawnApp(home: string): Promise<{ process: ChildProcess; close(): Promise<void> }> {
  const extra = process.env.ALTER_E2E_ARGS?.split(" ").filter(Boolean) ?? [];
  // 在 Node 里 import "electron" 得到的是 Electron 可执行文件的路径
  const electronPath = (await import("electron")).default as unknown as string;
  const child = spawn(electronPath, [MAIN_ENTRY, ...extra], {
    env: { ...process.env, ALTER_HOME: home },
    stdio: ["ignore", "pipe", "pipe"],
  });
  for (const stream of [child.stdout, child.stderr]) {
    stream.on("data", (chunk: Buffer) => logs.push(`[pid ${child.pid}] ${chunk.toString().trimEnd()}`));
  }
  if (!(await waitUntil(15_000, () => existsSync(join(home, "gateway.json"))))) {
    child.kill();
    throw new Error("15 秒内没有写出 gateway.json");
  }
  return {
    process: child,
    close: () =>
      new Promise((done) => {
        if (child.exitCode !== null) return done();
        child.once("exit", () => done());
        child.kill("SIGTERM");
      }),
  };
}

/** 本用例期间启动的 app 的输出行（含转发的 alterd stderr）。 */
export function appLogs(): string[] {
  return logs.flatMap((chunk) => chunk.split("\n"));
}

/** 用例失败时打印本用例期间启动的 app 的输出；每个用例结束都清空。 */
export function dumpLogsOnFailure(testInfo: TestInfo): void {
  if (testInfo.status !== testInfo.expectedStatus && logs.length > 0) {
    console.log(`--- app 输出（${testInfo.title}）---\n${logs.join("\n")}`);
  }
  logs.length = 0;
}

/** 每 50 ms 检查一次，直到条件成立或超时；返回条件最终是否成立。 */
export async function waitUntil(timeoutMs: number, cond: () => boolean): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (cond()) return true;
    await new Promise((r) => setTimeout(r, 50));
  }
  return cond();
}

/** pid 对应的进程是否存活。 */
export function isAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return (e as NodeJS.ErrnoException).code === "EPERM";
  }
}
