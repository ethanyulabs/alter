// ESLint 扁平配置：TypeScript 推荐规则，忽略生成文件与构建产物
import js from "@eslint/js";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "**/node_modules/**",
      "**/out/**",
      "**/dist/**",
      "target/**",
      // scripts/dist.sh 的中间产物（打包好的 SDK 运行时等）
      "build/**",
      "packages/protocol/src/generated/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
);
