// preload 注入到渲染进程的全局对象；vite/client 提供 .css 等资源模块的类型
/// <reference types="vite/client" />
import type { AlterApi } from "../../shared/daemon";
import type { ShellApi } from "../../shared/shell";

declare global {
  interface Window {
    alter: AlterApi & ShellApi;
  }
}

export {};
