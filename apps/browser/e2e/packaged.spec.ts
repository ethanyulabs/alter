// N2 release：打包后的 alter.app 用包内的 alterd、alter、bun 与 SDK 运行时。
// 先 `bun run dist:dir` 产出 dist/mac-arm64/alter.app；没有这个目录时跳过（默认的 `bun run e2e` 不打包）
import { execFileSync } from "node:child_process";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { _electron as electron, expect, test } from "@playwright/test";

import { dumpLogsOnFailure, tempHome, waitUntil } from "./app";

const APP = resolve(__dirname, "../../../dist/mac-arm64/alter.app");
const RES = join(APP, "Contents/Resources");

// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
});

test("RC N2 R2.S1 打包后的 app 拉起包内的 alterd，包内的 alter nodejs 能跑脚本", async () => {
  test.skip(!existsSync(APP), `没有 ${APP}：先运行 bun run dist:dir`);
  const home = tempHome();
  // 包内是 release 构建的 alterd，默认会在启动 5 秒后访问 GitHub 检查更新；测试不联网，关掉它。
  // 检查地址指向本地记录请求的服务器、延迟设为 0：配置没有生效时，用例结束前就会记到请求
  writeFileSync(join(home, "config.toml"), "update_check = false\n");
  const checks: string[] = [];
  const releases = createServer((req, res) => {
    checks.push(req.url ?? "");
    res.writeHead(404).end();
  });
  await new Promise<void>((ok) => releases.listen(0, "127.0.0.1", ok));
  const env: Record<string, string> = {
    ...(process.env as Record<string, string>),
    ALTER_HOME: home,
    ALTER_RELEASE_API: `http://127.0.0.1:${(releases.address() as AddressInfo).port}`,
    ALTER_UPDATE_CHECK_DELAY_MS: "0",
  };
  // FORCE_COLOR 由 Playwright 设置：Bun 会给打印的布尔值加颜色转义码
  for (const key of ["ALTER_ALTERD_PATH", "ALTER_BUN_PATH", "ALTER_SDK_RUNTIME", "ELECTRON_RENDERER_URL", "FORCE_COLOR"]) {
    delete env[key];
  }
  const app = await electron.launch({ executablePath: join(APP, "Contents/MacOS/alter"), args: [], env });
  try {
    const health = join(home, "alterd.json");
    expect(await waitUntil(10_000, () => existsSync(health)), "10 秒内应当出现 alterd.json").toBe(true);
    const pid = String(JSON.parse(readFileSync(health, "utf8")).pid);
    const comm = execFileSync("ps", ["-o", "comm=", "-p", pid], { encoding: "utf8" }).trim();
    expect(comm.endsWith("alter.app/Contents/Resources/bin/alterd"), `alterd 进程：${comm}`).toBe(true);

    expect(await waitUntil(10_000, () => existsSync(join(home, "gateway.json"))), "应当出现 gateway.json").toBe(true);
    // PATH 里只有系统目录：bun 与运行时只能来自包内
    const out = execFileSync(
      join(RES, "bin/alter"),
      ["nodejs", "-e", "console.log((await listTaskSpaces()).length >= 0)"],
      { encoding: "utf8", env: { ...env, PATH: "/usr/bin:/bin" } },
    );
    expect(out).toBe("true\n");
    // 上面的同步调用期间事件循环停着，留一点时间让可能到达的请求被记下
    await new Promise((ok) => setTimeout(ok, 500));
    expect(checks, "update_check = false 时不应检查更新").toEqual([]);
  } finally {
    await app.close();
    releases.close();
  }
});
