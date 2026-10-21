// 根组件：产品名与 alterd 状态
import { createSignal, onCleanup, onMount } from "solid-js";

import type { DaemonState, DaemonStatus } from "../../shared/daemon";

const LABELS: Record<DaemonState, string> = {
  starting: "alterd 启动中",
  running: "alterd 运行中",
  missing: "alterd 未找到",
  exited: "alterd 已退出",
};

export default function App() {
  const [status, setStatus] = createSignal<DaemonStatus>({ state: "starting" });

  onMount(() => {
    // 先订阅再取当前值，避免漏掉两者之间的推送
    const off = window.alter.onDaemonStatus(setStatus);
    void window.alter.getDaemonStatus().then(setStatus);
    onCleanup(off);
  });

  return (
    <main>
      <h1>alter</h1>
      <p>{LABELS[status().state]}</p>
    </main>
  );
}
