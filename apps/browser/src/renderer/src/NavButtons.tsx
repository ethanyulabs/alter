// 后退、前进、刷新：作用于当前激活标签，没有历史时对应按钮不可点
import type { JSX } from "solid-js";

import type { NavState } from "../../shared/shell";

interface Props {
  nav: NavState;
  hasTab: boolean;
}

export default function NavButtons(props: Props): JSX.Element {
  return (
    <div class="nav">
      <button
        data-testid="nav-back"
        title="后退"
        disabled={!props.nav.canGoBack}
        onClick={() => window.alter.navigate("back")}
      >
        ←
      </button>
      <button
        data-testid="nav-forward"
        title="前进"
        disabled={!props.nav.canGoForward}
        onClick={() => window.alter.navigate("forward")}
      >
        →
      </button>
      <button
        data-testid="nav-reload"
        title="刷新"
        disabled={!props.hasTab}
        onClick={() => window.alter.navigate("reload")}
      >
        ↻
      </button>
    </div>
  );
}
