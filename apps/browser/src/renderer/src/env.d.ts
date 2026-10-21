// preload 注入到渲染进程的全局对象
import type { AlterApi } from "../../shared/daemon";

declare global {
  interface Window {
    alter: AlterApi;
  }
}

export {};
