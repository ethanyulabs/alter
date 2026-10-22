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
