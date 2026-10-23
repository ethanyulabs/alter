// 侧边栏：产品名、当前 Space 的标签列表、底部的菜单（导入…）、Space 名称、Space 圆点与 alterd 状态
import { createSignal, For, onCleanup, onMount, Show, type JSX } from "solid-js";

import type { ShellState, TabView } from "@alter/protocol";

import { SIDEBAR_WIDTH, type NavState } from "../../shared/shell";
import AddressBar from "./AddressBar";
import ArchivedList from "./ArchivedList";
import HandoffBanner from "./HandoffBanner";
import ImportWizard from "./ImportWizard";
import NavButtons from "./NavButtons";
import SpaceBar from "./SpaceBar";
import UpdateBanner from "./UpdateBanner";

interface Props {
  state: ShellState | null;
  nav: NavState;
  error?: string;
  daemonLabel: string;
}

export default function Sidebar(props: Props): JSX.Element {
  const activeSpace = () => props.state?.spaces.find((s) => s.id === props.state?.activeSpaceId);
  const tabs = () => props.state?.tabs.filter((t) => t.spaceId === props.state?.activeSpaceId) ?? [];
  const pinnedTabs = () => tabs().filter((t) => t.pinned);
  const normalTabs = () => tabs().filter((t) => !t.pinned);

  /** 正在拖动的标签。 */
  const [dragging, setDragging] = createSignal<string | undefined>();

  /** 「已归档」列表是否展开。 */
  const [showArchived, setShowArchived] = createSignal(false);

  /** 底部菜单是否展开、导入向导是否打开。 */
  const [showMenu, setShowMenu] = createSignal(false);
  const [showImport, setShowImport] = createSignal(false);

  /** 行的右键菜单：所属标签与弹出位置（限制在侧边栏以内，页面视图会盖住超出的部分）。 */
  const [menu, setMenu] = createSignal<{ tab: TabView; x: number; y: number } | undefined>();
  const MENU_WIDTH = 180;
  onMount(() => {
    const close = () => {
      setMenu(undefined);
      setShowMenu(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("click", close);
    window.addEventListener("keydown", onKey);
    onCleanup(() => {
      window.removeEventListener("click", close);
      window.removeEventListener("keydown", onKey);
    });
  });

  /** 把正在拖的标签放到目标分区的第 `index` 位（index 不计被拖的标签本身）。 */
  const drop = (target: { pinned: boolean; index: number }) => {
    const tabId = dragging();
    setDragging(undefined);
    if (tabId) window.alter.moveTab({ tabId, ...target });
  };

  /** 放到分区末尾。index 不能超出协议的 u32，所以用该分区（不含被拖标签）的长度。 */
  const dropAtEnd = (pinned: boolean) => {
    const tabId = dragging();
    const section = (pinned ? pinnedTabs() : normalTabs()).filter((t) => t.id !== tabId);
    drop({ pinned, index: section.length });
  };

  /** 放在某一行上：落在上半部分插到它前面，下半部分插到它后面。 */
  const dropOnRow = (tab: TabView, e: DragEvent) => {
    const tabId = dragging();
    if (!tabId || tabId === tab.id) return;
    const section = (tab.pinned ? pinnedTabs() : normalTabs()).filter((t) => t.id !== tabId);
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const after = e.clientY > rect.top + rect.height / 2;
    drop({ pinned: tab.pinned, index: section.findIndex((t) => t.id === tab.id) + (after ? 1 : 0) });
  };

  const renderRow = (tab: TabView) => (
    <li
      data-testid="tab-row"
      data-tab-id={tab.id}
      data-pinned={String(tab.pinned)}
      classList={{ active: tab.active, dragging: dragging() === tab.id }}
      draggable={true}
      onClick={() => window.alter.activateTab(tab.id)}
      onDragStart={(e) => {
        setDragging(tab.id);
        e.dataTransfer?.setData("text/plain", tab.id);
        if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
      }}
      onDragEnd={() => setDragging(undefined)}
      onContextMenu={(e) => {
        e.preventDefault();
        const x = Math.min(e.clientX, SIDEBAR_WIDTH - MENU_WIDTH - 8);
        setMenu({ tab, x: Math.max(8, x), y: e.clientY });
      }}
      onDragOver={(e) => e.preventDefault()}
      onDrop={(e) => {
        e.preventDefault();
        dropOnRow(tab, e);
      }}
    >
      <Show when={tab.faviconUrl}>
        {(src) => <img class="favicon" data-testid="tab-favicon" src={src()} alt="" />}
      </Show>
      <Show when={tab.label}>
        {(label) => (
          <span class="agent-badge" data-testid="agent-badge" title="agent 页面">
            {label()}
          </span>
        )}
      </Show>
      <span class="title">{tab.title || tab.url}</span>
      {/* 图钉与「×」都由 CSS 伪元素画出，不计入行的文字 */}
      <button
        data-testid="tab-pin"
        class="pin"
        classList={{ on: tab.pinned }}
        aria-label={tab.pinned ? "取消固定" : "固定标签"}
        title={tab.pinned ? "取消固定" : "固定标签"}
        onClick={(e) => {
          e.stopPropagation();
          window.alter.pinTab({ tabId: tab.id, pinned: !tab.pinned });
        }}
      />
      <button
        data-testid="tab-close"
        class="close"
        aria-label="关闭标签"
        title="关闭标签"
        onClick={(e) => {
          e.stopPropagation();
          window.alter.closeTab(tab.id);
        }}
      />
    </li>
  );

  return (
    <aside class="sidebar" style={{ width: `${SIDEBAR_WIDTH}px` }}>
      <h1>alter</h1>
      <HandoffBanner state={props.state} />
      <NavButtons nav={props.nav} hasTab={tabs().some((t) => t.active)} />
      <AddressBar url={props.nav.url} error={props.error} />
      <ul class="tabs">
        <For each={pinnedTabs()}>{renderRow}</For>
        {/* 分隔线：上方是固定分区；拖到这里即固定到分区末尾 */}
        <li
          data-testid="pinned-divider"
          class="divider"
          classList={{ empty: pinnedTabs().length === 0 }}
          onDragOver={(e) => e.preventDefault()}
          onDrop={(e) => {
            e.preventDefault();
            dropAtEnd(true);
          }}
        />
        <For each={normalTabs()}>{renderRow}</For>
        {/* 普通分区末尾的空白处：拖到这里放到普通分区末尾（全部标签都固定时它是普通分区唯一的落点） */}
        <li
          data-testid="normal-drop"
          class="drop-end"
          onDragOver={(e) => e.preventDefault()}
          onDrop={(e) => {
            e.preventDefault();
            dropAtEnd(false);
          }}
        />
      </ul>
      <Show when={menu()}>
        {(m) => (
          <div
            class="tab-menu"
            data-testid="tab-menu"
            style={{ left: `${m().x}px`, top: `${m().y}px`, width: `${MENU_WIDTH}px` }}
            onClick={(e) => e.stopPropagation()}
          >
            <Show when={!m().tab.active}>
              <button
                data-testid="tab-menu-split"
                onClick={() => {
                  window.alter.splitTab(m().tab.id);
                  setMenu(undefined);
                }}
              >
                与当前标签分屏
              </button>
            </Show>
            <Show when={m().tab.splitGroup}>
              <button
                data-testid="tab-menu-unsplit"
                onClick={() => {
                  window.alter.unsplitTab(m().tab.id);
                  setMenu(undefined);
                }}
              >
                取消分屏
              </button>
            </Show>
          </div>
        )}
      </Show>
      <Show when={showArchived()}>
        <ArchivedList state={props.state} />
      </Show>
      <Show when={showImport()}>
        <ImportWizard state={props.state} onClose={() => setShowImport(false)} />
      </Show>
      <footer>
        <div class="sidebar-menu-wrap">
          <button
            class="sidebar-menu"
            data-testid="sidebar-menu"
            title="菜单"
            aria-label="菜单"
            onClick={(e) => {
              e.stopPropagation();
              setShowMenu(!showMenu());
            }}
          >
            ⋯
          </button>
          <Show when={showMenu()}>
            <div class="tab-menu sidebar-menu-list" data-testid="sidebar-menu-list" onClick={(e) => e.stopPropagation()}>
              <button
                data-testid="menu-import"
                onClick={() => {
                  setShowMenu(false);
                  setShowImport(true);
                }}
              >
                导入…
              </button>
            </div>
          </Show>
        </div>
        <button
          class="archived-toggle"
          data-testid="archived-toggle"
          classList={{ on: showArchived() }}
          onClick={() => setShowArchived(!showArchived())}
        >
          已归档
        </button>
        <span data-testid="space-name" style={{ color: activeSpace()?.color }}>
          {activeSpace()?.name ?? ""}
        </span>
        <UpdateBanner state={props.state} />
        <SpaceBar state={props.state} />
        <p>{props.daemonLabel}</p>
      </footer>
    </aside>
  );
}
