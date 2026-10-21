// Electron 端到端测试配置：每个用例自己启动 app，串行执行避免争用窗口
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  workers: 1,
  timeout: 30_000,
  reporter: "list",
});
