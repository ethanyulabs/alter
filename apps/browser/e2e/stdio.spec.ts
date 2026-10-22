// app-shell 能力：stdout / stderr 的读端关闭后，主进程照常运行
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { expect, test, type ElectronApplication } from "@playwright/test";

import { dumpLogsOnFailure, launchApp, tempHome, waitUntil } from "./app";

/** 等测试发话后向 stderr 写 256 KB、写完留下标记文件的假 alterd。 */
const CHATTY_ALTERD = resolve(__dirname, "fixtures/chatty-alterd.sh");

// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
});

/** `promise` 在 `ms` 内完成则返回它的值，否则返回 "timeout"。 */
async function within<T>(ms: number, promise: Promise<T>): Promise<T | "timeout"> {
  return Promise.race([promise, new Promise<"timeout">((r) => setTimeout(() => r("timeout"), ms))]);
}

/**
 * 关闭 app 某个输出管道的读端，等它真正关闭（`close`）：`destroy()` 是异步的，
 * 紧接着的写入可能还写进了管道缓冲区，不会得到 EPIPE。
 */
async function closeReader(stream: NodeJS.ReadableStream & { destroy(): void } | null): Promise<void> {
  if (!stream) throw new Error("app 的输出不是管道");
  const closed = new Promise((r) => stream.once("close", r));
  stream.destroy();
  await closed;
}

/**
 * 关闭 app；主进程被未捕获异常的原生弹窗卡住时 close 不会返回，3 秒后直接杀掉（alterd 随 stdin 关闭退出），
 * 不留下孤儿窗口。
 */
async function closeOrKill(app: ElectronApplication): Promise<void> {
  if ((await within(3_000, app.close())) === "timeout") app.process().kill("SIGKILL");
}

test("R1.S1 stdio：stdout/stderr 的读端关闭后，console 输出与进程警告不产生未捕获异常", async () => {
  const home = tempHome();
  const record = join(home, "uncaught.log");
  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    await expect(win.getByText("alterd 运行中")).toBeVisible({ timeout: 5_000 });
    // uncaughtExceptionMonitor 只旁听、不改变默认处理（弹窗）。同步写文件：弹窗会卡住主进程，之后读不到内存里的记录
    await app.evaluate((_electron, path) => {
      const fs = process.getBuiltinModule("node:fs");
      process.on("uncaughtExceptionMonitor", (e) => fs.appendFileSync(path, `${String(e)}\n`));
    }, record);
    // 关闭读端：之后主进程往 stdout / stderr 写都会得到 EPIPE
    await closeReader(app.process().stdout);
    await closeReader(app.process().stderr);
    // 直接写 stdout 单独一步：Node 的 Console 在写入失败的回调里会给没有监听器的流补一个一次性的空 error 监听，
    // 同一时刻的 console 输出会顺带兜住直接写入的错误，测不出 stdout 有没有防护
    await app.evaluate(() => {
      process.stdout.write("[stdio-e2e] stdout write\n");
    });
    // 写失败的 error 事件异步发出：给它 500 毫秒
    await new Promise((r) => setTimeout(r, 500));
    // 主进程若已被未捕获异常的弹窗卡住，这一步不会返回：限时，好让下面的断言报出原因。
    // 截止时间在发起调用之前取，完成后再核对一次
    const consoleDeadline = performance.now() + 2_000;
    const consoleStep = await within(
      Math.max(1, consoleDeadline - performance.now()),
      app.evaluate(() => {
        console.log("[stdio-e2e] stdout");
        console.error("[stdio-e2e] stderr");
        process.emitWarning("[stdio-e2e] warning");
      }),
    );
    const consoleDone = performance.now();
    await new Promise((r) => setTimeout(r, 1_000));
    expect(existsSync(record) ? readFileSync(record, "utf8") : "", "主进程出现了未捕获异常").toBe("");
    expect(consoleStep, "写日志那一步主进程应当在 2 秒内响应").not.toBe("timeout");
    expect(consoleDone).toBeLessThanOrEqual(consoleDeadline);
    const pingDeadline = performance.now() + 2_000;
    const ping = await within(
      Math.max(1, pingDeadline - performance.now()),
      app.evaluate(() => 1 + 1),
    );
    expect(ping, "主进程应当仍能响应").toBe(2);
    expect(performance.now()).toBeLessThanOrEqual(pingDeadline);
  } finally {
    await closeOrKill(app);
  }
});

test("R1.S2 stdio：stderr 的读端关闭后，alterd 写出的 256 KB 日志 3 秒内被读走", async () => {
  const home = tempHome();
  const app = await launchApp({ ALTER_HOME: home, ALTER_ALTERD_PATH: CHATTY_ALTERD });
  try {
    const win = await app.firstWindow();
    await expect(win.getByText("alterd 运行中")).toBeVisible({ timeout: 5_000 });
    await closeReader(app.process().stderr);
    // 截止时间在让假 alterd 开始写之前取
    const deadline = performance.now() + 3_000;
    writeFileSync(join(home, "start-logging"), "");
    const done = await waitUntil(Math.max(1, deadline - performance.now()), () =>
      existsSync(join(home, "stderr-done")),
    );
    expect(done, "alterd 的 stderr 没人读，写满管道后卡住").toBe(true);
    expect(performance.now()).toBeLessThanOrEqual(deadline);
  } finally {
    await closeOrKill(app);
  }
});


test("stdio 背压：app 的 stderr 读得慢时，主进程不替 alterd 缓存日志；恢复读取后 3 秒内写完", async () => {
  const home = tempHome();
  // 4 MB：沿途每一级管道与流缓冲各约 64 KB，合计远小于它；没有背压时主进程会把它全部吃进内存
  const app = await launchApp({ ALTER_HOME: home, ALTER_ALTERD_PATH: CHATTY_ALTERD, CHATTY_BYTES: String(4 * 1024 * 1024) });
  try {
    const win = await app.firstWindow();
    await expect(win.getByText("alterd 运行中")).toBeVisible({ timeout: 5_000 });
    // 读端还在，只是暂停读取：有背压时假 alterd 写不完
    const stderr = app.process().stderr;
    if (!stderr) throw new Error("app 的 stderr 不是管道");
    stderr.pause();
    writeFileSync(join(home, "start-logging"), "");
    await new Promise((r) => setTimeout(r, 2_000));
    expect(existsSync(join(home, "stderr-done")), "主进程替 alterd 缓存了日志（没有背压）").toBe(false);
    // 截止时间在恢复读取之前取
    const deadline = performance.now() + 3_000;
    stderr.resume();
    const done = await waitUntil(Math.max(1, deadline - performance.now()), () =>
      existsSync(join(home, "stderr-done")),
    );
    expect(done, "恢复读取后 alterd 的日志应当写完").toBe(true);
    expect(performance.now()).toBeLessThanOrEqual(deadline);
  } finally {
    await closeOrKill(app);
  }
});
