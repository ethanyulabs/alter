// 根组件：左侧侧边栏，右侧内容区（标签页面由主进程用 WebContentsView 覆盖在这里）
import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { createStore, reconcile } from "solid-js/store";

import type { DaemonState, DaemonStatus } from "../../shared/daemon";
import type { ShellView } from "../../shared/shell";
import CommandBar from "./CommandBar";
import PeekChrome from "./PeekChrome";
import Sidebar from "./Sidebar";

const LABELS: Record<DaemonState, string> = {
  starting: "alterd 启动中",
  running: "alterd 运行中",
  missing: "alterd 未找到",
  exited: "alterd 已退出",
};

export default function App() {
  const [status, setStatus] = createSignal<DaemonStatus>({ state: "starting" });
  // 快照按 id 合并进 store：同一个标签 / Space 保持同一个对象，列表行不会在每份快照里被整体重建
  // （重建会打断正在进行的拖拽，也会丢掉行上的 hover 状态）
  const [view, setView] = createStore<ShellView>({
    state: null,
    nav: { canGoBack: false, canGoForward: false, url: "" },
  });
  const applyView = (next: ShellView) => setView(reconcile(next, { key: "id" }));

  onMount(() => {
    // 先订阅再取当前值，避免漏掉两者之间的推送
    const offDaemon = window.alter.onDaemonStatus(setStatus);
    const offShell = window.alter.onShellView(applyView);
    void window.alter.getDaemonStatus().then(setStatus);
    void window.alter.getShellView().then(applyView);
    onCleanup(() => {
      offDaemon();
      offShell();
    });
  });

  const hasActiveTab = () => {
    const s = view.state;
    return s?.tabs.some((t) => t.spaceId === s.activeSpaceId && t.active) ?? false;
  };

  return (
    <div class="layout">
      <Sidebar
        state={view.state}
        nav={view.nav}
        error={view.error?.message}
        daemonLabel={LABELS[status().state]}
      />
      <main class="content">
        <Show when={!hasActiveTab()}>
          <p data-testid="empty-content">没有打开的标签</p>
        </Show>
        <PeekChrome />
        <CommandBar />
      </main>
    </div>
  );
}
