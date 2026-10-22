// agent 相关 e2e 的辅助：用仓库里构建出的 `alter nodejs` 运行脚本（ALTER_HOME 与 app 相同）
import { spawn } from "node:child_process";
import { resolve } from "node:path";

/** 仓库内构建出的 alter CLI（`bun run e2e` 会先 cargo build）。 */
export const ALTER = resolve(__dirname, "../../../target/debug/alter");

export interface ScriptResult {
  stdout: string;
  stderr: string;
  code: number | null;
}

/** 运行 `alter nodejs -e <code>`；`env` 追加环境变量。 */
export function runScript(home: string, code: string, env: Record<string, string> = {}): Promise<ScriptResult> {
  return new Promise((done) => {
    const child = spawn(ALTER, ["nodejs", "-e", code], {
      // Playwright 的环境带 FORCE_COLOR，Bun 会给 console.log 的数字上色：输出要的是纯文本
      env: {
        ...process.env,
        FORCE_COLOR: "0",
        NO_COLOR: "1",
        ALTER_HOME: home,
        ALTER_BUN_PATH: process.execPath.endsWith("bun") ? process.execPath : "bun",
        ...env,
      },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (c: Buffer) => (stdout += c.toString()));
    child.stderr.on("data", (c: Buffer) => (stderr += c.toString()));
    child.on("close", (code) => done({ stdout, stderr, code }));
  });
}
