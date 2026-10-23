// RC N2 R13：alter nodejs 运行的脚本收到 app.updateAvailable 时，stderr 恰好打印一次提示
import { expect, test } from "bun:test";
import { join } from "node:path";

import { fakeGateway } from "./fake-gateway";

const RUNTIME = join(import.meta.dir, "../src/runtime.ts");

test("RC N2 R13.S1 收到两次通知只打印一次", async () => {
  // 在回 taskSpace.list 之前推两次通知：脚本拿到结果时两条都已到达
  const gw = fakeGateway((req) => {
    if (req.method === "taskSpace.list") {
      gw.push("app.updateAvailable", { version: "9.0.0", url: "https://example.invalid/r/v9.0.0" });
      gw.push("app.updateAvailable", { version: "9.0.0", url: "https://example.invalid/r/v9.0.0" });
      return { result: { taskSpaces: [] } };
    }
    return { result: {} };
  });
  try {
    const dir = (await import("node:fs")).mkdtempSync(join((await import("node:os")).tmpdir(), "alter-notice-"));
    const script = join(dir, "script.ts");
    await Bun.write(script, "await listTaskSpaces();\nawait Bun.sleep(100);\n");
    const proc = Bun.spawn(["bun", "--preload", RUNTIME, script], {
      env: { ...process.env, ALTER_GATEWAY_FILE: gw.gatewayFile },
      stdout: "pipe",
      stderr: "pipe",
    });
    const code = await proc.exited;
    const stderr = await new Response(proc.stderr).text();
    expect(code).toBe(0);
    const lines = stderr.split("\n").filter((l) => l.includes("[alter:notice]"));
    expect(lines).toEqual(["[alter:notice] alter 9.0.0 is available, run: alter upgrade"]);
  } finally {
    gw.server.stop(true);
  }
});
