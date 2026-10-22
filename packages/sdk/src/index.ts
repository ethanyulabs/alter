// @alter/sdk：TaskSpace / Page API，在 Bun 里运行；与 ego-browser 的 API 形状一致

/** 包名，供日志与错误信息标识来源。 */
export const SDK_NAME = "@alter/sdk";

export { createAlter, type Alter, type AlterOptions } from "./alter";
export { AlterError } from "./errors";
export { Page, type ActionOptions, type GotoOptions, type WaitCondition } from "./page";
export { TaskSpace } from "./task-space";
