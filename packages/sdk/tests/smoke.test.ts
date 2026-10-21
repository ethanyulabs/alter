// 冒烟测试：证明 bun test 能发现并运行 workspace 内的测试。
import { expect, test } from "bun:test";

import pkg from "../package.json";
import { SDK_NAME } from "../src/index";

test("SDK_NAME 与 package.json 的 name 一致", () => {
  expect(SDK_NAME).toBe(pkg.name);
});
