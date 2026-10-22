// 命令栏：Cmd+T 打开，一个输入框搜索标签、Space、书签、历史，或打开 / 搜索输入的内容
import { createEffect, createSignal, For, onCleanup, onMount, Show, type JSX } from "solid-js";

import type { SearchItem } from "@alter/protocol";

const KIND_LABEL: Record<SearchItem["kind"], string> = {
  tab: "标签",
  space: "Space",
  bookmark: "书签",
  history: "历史",
  open: "打开",
};

export default function CommandBar(): JSX.Element {
  let input: HTMLInputElement | undefined;
  const [open, setOpen] = createSignal(false);
  const [items, setItems] = createSignal<SearchItem[]>([]);
  const [selected, setSelected] = createSignal(0);
  /** 最近一次发出的搜索；只接受它的结果，丢弃过期的 */
  let latest = 0;
  /** 当前显示的结果对应的搜索 */
  let shown = 0;
  /** 结果还没回来就按了回车：等结果到了执行第一项 */
  let pendingEnter = false;

  const close = () => {
    setOpen(false);
    pendingEnter = false;
    window.alter.closeCommandBar();
  };
  const run = (item: SearchItem | undefined) => {
    if (!item) return;
    setOpen(false);
    pendingEnter = false;
    window.alter.executeCommand(item);
  };

  onMount(() => {
    const offOpen = window.alter.onCommandBarOpen(() => {
      latest += 1;
      shown = latest;
      pendingEnter = false;
      setItems([]);
      setSelected(0);
      if (input) input.value = "";
      setOpen(true);
    });
    const offResults = window.alter.onSearchResults(({ requestId, items: next }) => {
      if (requestId !== latest) return;
      shown = requestId;
      setItems(next);
      setSelected(0);
      if (pendingEnter) run(next[0]);
    });
    onCleanup(() => {
      offOpen();
      offResults();
    });
  });

  // 打开时聚焦输入框
  createEffect(() => {
    if (open()) queueMicrotask(() => input?.focus());
  });

  const onInput = () => {
    latest += 1;
    window.alter.search({ requestId: latest, query: input?.value ?? "" });
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const n = items().length;
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "ArrowDown" && n > 0) {
      e.preventDefault();
      setSelected((selected() + 1) % n);
    } else if (e.key === "ArrowUp" && n > 0) {
      e.preventDefault();
      setSelected((selected() - 1 + n) % n);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (shown === latest) run(items()[selected()]);
      else pendingEnter = true;
    }
  };

  return (
    <Show when={open()}>
      <div class="command-backdrop" onClick={close}>
        <div class="command-bar" data-testid="command-bar" onClick={(e) => e.stopPropagation()}>
          <input
            ref={(el) => (input = el)}
            data-testid="command-input"
            placeholder="搜索标签、Space、历史，或输入网址"
            spellcheck={false}
            onInput={onInput}
            onKeyDown={onKeyDown}
          />
          <ul class="command-items">
            <For each={items()}>
              {(item, i) => (
                <li
                  data-testid="command-item"
                  data-kind={item.kind}
                  aria-selected={i() === selected() ? "true" : "false"}
                  classList={{ selected: i() === selected() }}
                  onMouseEnter={() => setSelected(i())}
                  onClick={() => run(item)}
                >
                  <span class="kind">{KIND_LABEL[item.kind]}</span>
                  <span class="title">{item.kind === "open" ? `打开或搜索：${item.title}` : item.title}</span>
                  <Show when={item.url && item.kind !== "open"}>
                    <span class="url">{item.url}</span>
                  </Show>
                </li>
              )}
            </For>
          </ul>
        </div>
      </div>
    </Show>
  );
}
