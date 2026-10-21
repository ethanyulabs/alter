// 地址框与「新标签」按钮：点「新标签」后进入新标签模式，回车提交给主进程
import { createEffect, createSignal, Show, type JSX } from "solid-js";

interface Props {
  /** 当前激活标签的 URL，地址框未编辑时显示它 */
  url: string;
  /** alterd 最近一次拒绝操作的说明；下一份快照到来时清空 */
  error?: string;
}

export default function AddressBar(props: Props): JSX.Element {
  let input: HTMLInputElement | undefined;
  const [newTab, setNewTab] = createSignal(false);

  // 激活标签的 URL 变化时同步显示；用户正在输入时不覆盖
  createEffect(() => {
    const url = props.url;
    if (input && document.activeElement !== input) input.value = url;
  });

  const startNewTab = () => {
    setNewTab(true);
    if (!input) return;
    input.value = "";
    input.focus();
  };

  const submit = (e: KeyboardEvent) => {
    if (e.key !== "Enter") return;
    const value = input?.value.trim() ?? "";
    if (value === "") return;
    window.alter.open({ input: value, newTab: newTab() });
    setNewTab(false);
    input?.blur();
  };

  return (
    <div class="address">
      <input
        ref={(el) => (input = el)}
        data-testid="address-input"
        placeholder={newTab() ? "新标签：输入网址或搜索" : "输入网址或搜索"}
        onKeyDown={submit}
        onBlur={() => setNewTab(false)}
      />
      <button data-testid="new-tab" onMouseDown={(e) => e.preventDefault()} onClick={startNewTab}>
        ＋ 新标签
      </button>
      <Show when={props.error}>
        {(message) => (
          <p class="error" data-testid="shell-error">
            {message()}
          </p>
        )}
      </Show>
    </div>
  );
}
