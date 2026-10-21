// app-shell 能力：app 被强制结束、或已有 alterd 在运行时的托管行为
import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { expect, test } from "@playwright/test";

import { isAlive, launchApp, tempHome, waitUntil } from "./app";

/** 仓库内构建出的 alterd（`bun run e2e` 会先 cargo build）。 */
const ALTERD = resolve(__dirname, "../../../target/debug/alterd");

function healthPath(home: string): string {
  return join(home, "alterd.json");
}

function readPid(home: string): number {
  return JSON.parse(readFileSync(healthPath(home), "utf8")).pid as number;
}

test("R2.S1 SIGKILL app 后 alterd 退出：2 秒内 pid 不再存活，健康文件已删除", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  let pid: number | undefined;
  try {
    const win = await app.firstWindow();
    await expect(win.getByText("alterd 运行中")).toBeVisible();
    pid = readPid(home);
    expect(isAlive(pid)).toBe(true);

    // 强制结束后 Playwright 的 app.close() 会报错，这里不再调用它
    app.process().kill("SIGKILL");
    const alive = pid;
    expect(await waitUntil(2_000, () => !isAlive(alive))).toBe(true);
    expect(existsSync(healthPath(home))).toBe(false);
  } finally {
    // 兜底：无论断言结果如何，不留下 alterd 进程
    if (pid !== undefined && isAlive(pid)) process.kill(pid, "SIGKILL");
  }
});

test("R3.S1 已有实例时复用：页面显示 alterd 运行中且 pid 为外部实例，app 退出后外部实例仍在", async () => {
  const home = tempHome();
  // 在 app 之外先拉起一个 alterd，stdin 为 /dev/null，不带新参数
  const external = spawn(ALTERD, ["run"], {
    env: { ...process.env, ALTER_HOME: home },
    stdio: "ignore",
  });
  const externalPid = external.pid!;
  try {
    expect(await waitUntil(3_000, () => existsSync(healthPath(home)))).toBe(true);
    expect(readPid(home)).toBe(externalPid);

    const app = await launchApp({ ALTER_HOME: home });
    try {
      const win = await app.firstWindow();
      await expect(win.getByText("alterd 运行中")).toBeVisible({ timeout: 3_000 });
      const status = await win.evaluate(() => window.alter.getDaemonStatus());
      expect(status.pid).toBe(externalPid);
    } finally {
      await app.close();
    }

    await new Promise((r) => setTimeout(r, 2_000));
    expect(isAlive(externalPid)).toBe(true);
    expect(existsSync(healthPath(home))).toBe(true);
    expect(readPid(home)).toBe(externalPid);
  } finally {
    // 外部实例由用例自己拉起，也由用例自己结束
    if (isAlive(externalPid)) process.kill(externalPid, "SIGKILL");
  }
});
