// 已归档列表：当前 Space 最近归档的标签，点一项恢复（放回普通分区末尾并激活）
import { For, Show, type JSX } from "solid-js";

import type { ShellState } from "@alter/protocol";

interface Props {
  state: ShellState | null;
}

export default function ArchivedList(props: Props): JSX.Element {
  const items = () => props.state?.archived.filter((a) => a.spaceId === props.state?.activeSpaceId) ?? [];
  return (
    <div class="archived-list" data-testid="archived-list">
      <Show when={items().length > 0} fallback={<p class="empty">没有已归档的标签</p>}>
        <ul>
          <For each={items()}>
            {(item) => (
              <li data-testid="archived-item" title={item.url} onClick={() => window.alter.restoreTab(item.id)}>
                {item.title || item.url}
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
}
