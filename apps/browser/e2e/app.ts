// 启动打包前 app 的公共函数
import { mkdtempSync } from "node:fs";
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
  // 收集主进程（含转发的 alterd stderr）的输出，用例失败时由 dumpLogsOnFailure 打印
  const pid = app.process().pid;
  for (const stream of [app.process().stdout, app.process().stderr]) {
    stream?.on("data", (chunk: Buffer) => logs.push(`[pid ${pid}] ${chunk.toString().trimEnd()}`));
  }
  app.process().on("exit", (code, signal) => logs.push(`[pid ${pid}] exited code=${code} signal=${signal}`));
  return app;
}

const logs: string[] = [];

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
