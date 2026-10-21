// electron-vite 配置：main / preload 输出 CommonJS（preload 在沙箱里只能是 CJS），renderer 用 SolidJS
import { defineConfig } from "electron-vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  main: {},
  preload: {},
  renderer: {
    plugins: [solid()],
  },
});
