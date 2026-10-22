// 侧边栏底部的 Space 圆点行：点击切换，右键编辑，「+」新建
import type { ShellState, SpaceView } from "@alter/protocol";
import { createSignal, For, Show, type JSX } from "solid-js";

import SpacePanel from "./SpacePanel";

interface Props {
  state: ShellState | null;
}

/** 打开的面板：新建，或编辑某个 Space。 */
type Panel = { kind: "create" } | { kind: "edit"; space: SpaceView };

export default function SpaceBar(props: Props): JSX.Element {
  const [panel, setPanel] = createSignal<Panel | undefined>();
  const spaces = () => props.state?.spaces ?? [];

  return (
    <div class="space-bar">
      <Show when={panel()} keyed>
        {(p) => (
          <SpacePanel
            space={p.kind === "edit" ? p.space : undefined}
            canDelete={spaces().length > 1}
            onClose={() => setPanel(undefined)}
          />
        )}
      </Show>
      <div class="dots">
        <For each={spaces()}>
          {(space) => (
            <button
              data-testid="space-dot"
              classList={{ active: space.id === props.state?.activeSpaceId }}
              style={{ background: space.color }}
              title={space.name}
              aria-label={space.name}
              onClick={() => window.alter.activateSpace(space.id)}
              onContextMenu={(e) => {
                e.preventDefault();
                setPanel({ kind: "edit", space });
              }}
            />
          )}
        </For>
        <button data-testid="space-add" class="add" title="新建 Space" aria-label="新建 Space" onClick={() => setPanel({ kind: "create" })}>
          +
        </button>
      </div>
    </div>
  );
}
