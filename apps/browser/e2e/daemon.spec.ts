// app-shell 能力：守护进程托管
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { expect, test } from "@playwright/test";

import { isAlive, launchApp, tempHome, waitUntil } from "./app";

/** 3.5 秒后才写健康文件的假 alterd。 */
const SLOW_ALTERD = resolve(__dirname, "fixtures/slow-alterd.sh");

function healthPath(home: string): string {
  return join(home, "alterd.json");
}

function readPid(home: string): number {
  return JSON.parse(readFileSync(healthPath(home), "utf8")).pid as number;
}

test("R7.S1 启动拉起 alterd：3 秒内出现健康文件，pid 存活，页面显示 alterd 运行中", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  let pid: number | undefined;
  try {
    expect(await waitUntil(3_000, () => existsSync(healthPath(home)))).toBe(true);
    pid = readPid(home);
    expect(isAlive(pid)).toBe(true);
    const win = await app.firstWindow();
    await expect(win.getByText("alterd 运行中")).toBeVisible();
  } finally {
    await app.close();
    // 兜底：无论断言结果如何，不留下 alterd 进程
    if (pid !== undefined && isAlive(pid)) process.kill(pid, "SIGKILL");
  }
});

test("R7.S2 退出关闭 alterd：关闭 app 后 2 秒内 pid 不再存活，健康文件已删除", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home });
  let pid: number | undefined;
  try {
    expect(await waitUntil(3_000, () => existsSync(healthPath(home)))).toBe(true);
    pid = readPid(home);
    expect(isAlive(pid)).toBe(true);

    // 2 秒从发起关闭时算起，app 自己等待 alterd 退出的时间也计算在内
    const start = Date.now();
    await app.close();
    const alive = pid;
    const remaining = Math.max(0, 2_000 - (Date.now() - start));
    expect(await waitUntil(remaining, () => !isAlive(alive))).toBe(true);
    expect(existsSync(healthPath(home))).toBe(false);
    expect(Date.now() - start).toBeLessThanOrEqual(2_000);
  } finally {
    await app.close().catch(() => {});
    if (pid !== undefined && isAlive(pid)) process.kill(pid, "SIGKILL");
  }
});

test("R7.S3 alterd 二进制缺失：窗口仍打开，显示 alterd 未找到，5 秒后 app 仍在运行", async () => {
  const app = await launchApp({ ALTER_ALTERD_PATH: "/nonexistent/alterd" });
  try {
    const win = await app.firstWindow();
    await expect(win.getByText("alterd 未找到")).toBeVisible();

    await new Promise((r) => setTimeout(r, 5_000));
    const pid = app.process().pid;
    expect(pid).toBeDefined();
    expect(isAlive(pid!)).toBe(true);
    expect(app.process().exitCode).toBeNull();
  } finally {
    await app.close().catch(() => {});
  }
});

test("R7.S1 迟到就绪：alterd 超过 3 秒才写出健康文件，页面最终显示 alterd 运行中", async () => {
  const app = await launchApp({ ALTER_ALTERD_PATH: SLOW_ALTERD });
  try {
    const win = await app.firstWindow();
    await expect(win.getByText("alterd 运行中")).toBeVisible({ timeout: 8_000 });
  } finally {
    await app.close();
  }
});
