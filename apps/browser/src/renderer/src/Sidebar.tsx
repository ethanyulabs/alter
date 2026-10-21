// 侧边栏：产品名、当前 Space 的标签列表、底部的 Space 名称与 alterd 状态
import { For, Show, type JSX } from "solid-js";

import type { ShellState } from "@alter/protocol";

import { SIDEBAR_WIDTH, type NavState } from "../../shared/shell";
import AddressBar from "./AddressBar";
import NavButtons from "./NavButtons";

interface Props {
  state: ShellState | null;
  nav: NavState;
  error?: string;
  daemonLabel: string;
}

export default function Sidebar(props: Props): JSX.Element {
  const activeSpace = () => props.state?.spaces.find((s) => s.id === props.state?.activeSpaceId);
  const tabs = () => props.state?.tabs.filter((t) => t.spaceId === props.state?.activeSpaceId) ?? [];

  return (
    <aside class="sidebar" style={{ width: `${SIDEBAR_WIDTH}px` }}>
      <h1>alter</h1>
      <NavButtons nav={props.nav} hasTab={tabs().some((t) => t.active)} />
      <AddressBar url={props.nav.url} error={props.error} />
      <ul class="tabs">
        <For each={tabs()}>
          {(tab) => (
            <li
              data-testid="tab-row"
              classList={{ active: tab.active }}
              onClick={() => window.alter.activateTab(tab.id)}
            >
              <Show when={tab.faviconUrl}>
                {(src) => <img class="favicon" data-testid="tab-favicon" src={src()} alt="" />}
              </Show>
              <span class="title">{tab.title || tab.url}</span>
              {/* 「×」由 CSS 伪元素画出，不计入行的文字 */}
              <button
                data-testid="tab-close"
                aria-label="关闭标签"
                title="关闭标签"
                onClick={(e) => {
                  e.stopPropagation();
                  window.alter.closeTab(tab.id);
                }}
              />
            </li>
          )}
        </For>
      </ul>
      <footer>
        <span data-testid="space-name" style={{ color: activeSpace()?.color }}>
          {activeSpace()?.name ?? ""}
        </span>
        <p>{props.daemonLabel}</p>
      </footer>
    </aside>
  );
}
