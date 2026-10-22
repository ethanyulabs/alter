// 接管：当前 Space 里交给用户的 TaskSpace 显示横幅与「交还给 Agent」；agent 正在当前 Space 操作页面时显示「停止 Agent」
import { For, type JSX } from "solid-js";

import type { ShellState } from "@alter/protocol";

interface Props {
  state: ShellState | null;
}

export default function HandoffBanner(props: Props): JSX.Element {
  const here = () => props.state?.taskSpaces.filter((t) => t.spaceId === props.state?.activeSpaceId) ?? [];
  const waiting = () => here().filter((t) => t.ownership === "user");
  /** agent 控制、且当前 Space 里有它的页面 */
  const running = () =>
    here().filter((t) => t.ownership === "agent" && props.state?.tabs.some((tab) => tab.taskSpaceId === t.id));

  return (
    <>
      <For each={waiting()}>
        {(t) => (
          <div class="handoff-banner" data-testid="handoff-banner" data-task-space-id={t.id}>
            <span>等待你操作：{t.name}</span>
            <button data-testid="handoff-return" onClick={() => window.alter.returnTaskSpace(t.id)}>
              交还给 Agent
            </button>
          </div>
        )}
      </For>
      <For each={running()}>
        {(t) => (
          <button
            class="stop-agent"
            data-testid="stop-agent"
            data-task-space-id={t.id}
            title={`停止 Agent：${t.name}`}
            onClick={() => window.alter.stopTaskSpace(t.id)}
          >
            停止 Agent
          </button>
        )}
      </For>
    </>
  );
}
