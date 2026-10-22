// 父级 SC4：500 个标签时切换 Space 的界面耗时；SC3：2000 个元素的页面 page.snapshot 耗时
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createConnection } from "node:net";
import { join, resolve } from "node:path";

import { expect, test } from "@playwright/test";

import { runScript } from "./agent";
import { dumpLogsOnFailure, launchApp, tempHome, waitUntil } from "./app";
import { startServer, type TestServer } from "./fixtures/server";
import { ready, waitUntilAsync } from "./ui";

/** 仓库内构建出的 alterd（`bun run e2e` 会先 cargo build）。 */
const ALTERD = resolve(__dirname, "../../../target/debug/alterd");

/** release 版 alterd（`bun run e2e` 同样会先构建）：性能预算针对发布出去的二进制。 */
const ALTERD_RELEASE = resolve(__dirname, "../../../target/release/alterd");

const PER_SPACE = 250;

let server: TestServer;
test.beforeEach(async () => {
  server = await startServer();
});
// eslint-disable-next-line no-empty-pattern
test.afterEach(async ({}, testInfo) => {
  dumpLogsOnFailure(testInfo);
  await server.close();
});

interface Snapshot {
  spaces: { id: string; name: string }[];
  tabs: unknown[];
}

/** 不启动 app，直接拉起 alterd 经 shell.sock 建 Work，并给 Personal 与 Work 各开 250 个标签；返回两个 Space 的 id。 */
async function seed(home: string): Promise<string[]> {
  const alterd = spawn(ALTERD, ["run"], { env: { ...process.env, ALTER_HOME: home }, stdio: "ignore" });
  try {
    expect(await waitUntil(3_000, () => existsSync(join(home, "alterd.json")))).toBe(true);
    const socket = createConnection(join(home, "shell.sock"));
    socket.setEncoding("utf8");
    let buffer = "";
    let latest: Snapshot | undefined;
    socket.on("data", (chunk: string) => {
      buffer += chunk;
      let nl: number;
      while ((nl = buffer.indexOf("\n")) >= 0) {
        latest = JSON.parse(buffer.slice(0, nl)) as Snapshot;
        buffer = buffer.slice(nl + 1);
      }
    });
    const send = (event: object) => socket.write(`${JSON.stringify(event)}\n`);
    expect(await waitUntilAsync(2_000, async () => latest !== undefined)).toBe(true);
    send({ type: "shell.event.spaceCreated", name: "Work", color: "#f97316" });
    expect(await waitUntilAsync(2_000, async () => latest!.spaces.length === 2)).toBe(true);
    const ids = latest!.spaces.map((s) => s.id);
    // 两个 Space 的地址不同（Personal /n/0…，Work /n/1000…），计时才能确认列表真的换成了目标 Space 的
    ids.forEach((spaceId, k) => {
      for (let i = 0; i < PER_SPACE; i++) {
        send({ type: "shell.event.openRequested", spaceId, input: server.input(`/n/${k * 1000 + i}`) });
      }
    });
    expect(await waitUntilAsync(10_000, async () => latest?.tabs.length === 2 * PER_SPACE)).toBe(true);
    socket.end();
    return ids;
  } finally {
    const exited = new Promise((r) => alterd.once("exit", r));
    alterd.kill("SIGTERM");
    await exited;
  }
}

test("SC4 500 个标签时切换 Space：从 Renderer 发出切换到画出新列表，20 次的 p95 < 50 ms", async () => {
  const home = tempHome();
  const ids = await seed(home);
  const names = ["Personal", "Work"];

  const app = await launchApp({ ALTER_HOME: home });
  try {
    const win = await app.firstWindow();
    // 建 Work 后它是当前 Space
    await ready(win, "Work");
    await expect(win.getByTestId("tab-row")).toHaveCount(PER_SPACE, { timeout: 5_000 });

    // 计时在 Renderer 里完成：起点是调用 activateSpace（早于主进程收到 IPC，只会高估），
    // 终点是 requestAnimationFrame 回调里看到新 Space 的名称，且 250 行逐行都是目标 Space 的标签
    // 各 Space 第 i 行允许的文字：没加载过的页面显示 URL，加载过的（激活标签）显示标题
    const expected = [0, 1].map((k) =>
      Array.from({ length: PER_SPACE }, (_, i) => [server.url(`/n/${k * 1000 + i}`), `Page ${k * 1000 + i}`]),
    );
    const samples = await win.evaluate(
      async ({ ids, names, perSpace, expected }) => {
        const alter = (window as unknown as { alter: { activateSpace(id: string): void } }).alter;
        const out: number[] = [];
        for (let i = 0; i < 20; i++) {
          const target = i % 2;
          const t0 = performance.now();
          alter.activateSpace(ids[target]!);
          await new Promise<void>((done) => {
            const check = () => {
              const name = document.querySelector('[data-testid="space-name"]')?.textContent;
              const rows = [...document.querySelectorAll('[data-testid="tab-row"]')];
              const listed =
                rows.length === perSpace &&
                rows.every((r, i) => expected[target]![i]!.includes(r.textContent ?? ""));
              if (name === names[target] && listed) {
                out.push(performance.now() - t0);
                done();
              } else {
                requestAnimationFrame(check);
              }
            };
            requestAnimationFrame(check);
          });
          await new Promise((r) => setTimeout(r, 50));
        }
        return out;
      },
      { ids, names, perSpace: PER_SPACE, expected },
    );
    const sorted = [...samples].sort((a, b) => a - b);
    const p95 = sorted[Math.ceil(sorted.length * 0.95) - 1]!;
    console.log(`SC4: 20 次切换 p50 ${sorted[9]!.toFixed(1)} ms，p95 ${p95.toFixed(1)} ms，最大 ${sorted.at(-1)!.toFixed(1)} ms`);
    expect(p95).toBeLessThan(50);
  } finally {
    await app.close();
  }
});

test("SC3 2000 个元素的页面：10 次 page.snapshot 的 p95 < 150 ms", async () => {
  const home = tempHome();
  // debug 版 alterd 解析几 MB 的无障碍树要多花约 100 ms，不代表发布版本
  const app = await launchApp({ ALTER_HOME: home, ALTER_ALTERD_PATH: ALTERD_RELEASE });
  try {
    await ready(await app.firstWindow());
    // 计时在脚本里围住整个 SDK 调用（含 WebSocket 往返），只会高估
    const r = await runScript(
      home,
      `const p = (await taskSpace("t")).page("p1");
       await p.goto(${JSON.stringify(server.url("/big"))});
       await p.snapshot();
       const times = [];
       let refs = 0;
       for (let i = 0; i < 10; i++) {
         const t0 = performance.now();
         const s = await p.snapshot();
         times.push(performance.now() - t0);
         refs = s.split("[@e").length - 1;
       }
       console.log(JSON.stringify({ times, refs }));`,
    );
    expect(r.code, r.stderr).toBe(0);
    const { times, refs } = JSON.parse(r.stdout.trim()) as { times: number[]; refs: number };
    // 页面确实是 1000 个按钮（每个按钮一个 li 一个 button，共 2000 个元素）
    expect(refs).toBe(1000);
    const sorted = [...times].sort((a, b) => a - b);
    const p95 = sorted[Math.ceil(sorted.length * 0.95) - 1]!;
    console.log(`SC3: 10 次 snapshot p50 ${sorted[4]!.toFixed(1)} ms，p95 ${p95.toFixed(1)} ms`);
    expect(p95).toBeLessThan(150);
  } finally {
    await app.close();
  }
});
