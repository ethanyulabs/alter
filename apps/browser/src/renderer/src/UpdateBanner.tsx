// 更新提示条：alterd 检查到新版本时（快照里的 updateAvailable），在侧边栏底部 Space 圆点上方显示。
// 点提示条在当前 Space 新标签打开 Release 页；点「×」在本次运行内隐藏（记的是版本号，出了更新的版本会再显示）
import { createSignal, Show, type JSX } from "solid-js";

import type { ShellState } from "@alter/protocol";

interface Props {
  state: ShellState | null;
}

export default function UpdateBanner(props: Props): JSX.Element {
  const [dismissed, setDismissed] = createSignal<string | undefined>();
  const update = () => {
    const u = props.state?.updateAvailable;
    return u && u.version !== dismissed() ? u : undefined;
  };
  return (
    <Show when={update()}>
      {(u) => (
        <div class="update-banner" data-testid="update-banner">
          <button
            class="update-open"
            data-testid="update-open"
            title={u().url}
            onClick={() => window.alter.open({ input: u().url, newTab: true })}
          >
            新版本 {u().version} 可用 · 运行 alter upgrade
          </button>
          <button
            class="update-dismiss"
            data-testid="update-dismiss"
            aria-label="关闭更新提示"
            onClick={() => setDismissed(u().version)}
          >
            ×
          </button>
        </div>
      )}
    </Show>
  );
}
