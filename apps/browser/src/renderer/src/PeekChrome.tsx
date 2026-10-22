// Peek 的外框：内容区的半透明遮罩（点击关闭）与 Peek 上方的「在新标签打开」；Esc 关闭
import { createSignal, onCleanup, onMount, Show, type JSX } from "solid-js";

import { SIDEBAR_WIDTH, type PeekState } from "../../shared/shell";

export default function PeekChrome(): JSX.Element {
  const [state, setState] = createSignal<PeekState>({ open: false });
  onMount(() => {
    const off = window.alter.onPeekState(setState);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && state().open) window.alter.closePeek();
    };
    window.addEventListener("keydown", onKey);
    onCleanup(() => {
      off();
      window.removeEventListener("keydown", onKey);
    });
  });
  return (
    <Show when={state().open && state().bounds}>
      {(b) => (
        <div class="peek-backdrop" data-testid="peek-backdrop" onClick={() => window.alter.closePeek()}>
          <div
            class="peek-bar"
            style={{ left: `${b().x - SIDEBAR_WIDTH}px`, top: `${Math.max(4, b().y - 36)}px`, width: `${b().width}px` }}
            onClick={(e) => e.stopPropagation()}
          >
            <button data-testid="peek-promote" onClick={() => window.alter.promotePeek()}>
              在新标签打开
            </button>
            <button data-testid="peek-close" aria-label="关闭预览" onClick={() => window.alter.closePeek()}>
              ×
            </button>
          </div>
        </div>
      )}
    </Show>
  );
}
