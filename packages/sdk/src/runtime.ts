// alter nodejs 用 `bun --preload runtime.ts script` 加载：把 SDK 的入口函数挂到全局
import { createAlter } from "./alter";
import { AlterError } from "./errors";

const alter = createAlter();
const g = globalThis as Record<string, unknown>;
g["taskSpace"] = alter.taskSpace;
g["takeOverTaskSpace"] = alter.takeOverTaskSpace;
g["claimTaskSpace"] = alter.claimTaskSpace;
g["listTaskSpaces"] = alter.listTaskSpaces;
g["AlterError"] = AlterError;

// alterd 检查到新版本时推 app.updateAvailable：在 stderr 提示一次（stdout 留给脚本自己的输出）
let noticed = false;
alter.onNotification((method, params) => {
  if (method !== "app.updateAvailable" || noticed) return;
  noticed = true;
  console.error(`[alter:notice] alter ${String(params["version"])} is available, run: alter upgrade`);
});
