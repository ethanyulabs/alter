// 导入向导：选来源浏览器、profile、目标 Space 与数据类别，开始后显示进度与各类计数
import { createSignal, For, onCleanup, onMount, Show, type JSX } from "solid-js";

import type { ImportItem, ImportSource, ShellState } from "@alter/protocol";

import type { ImportProgressView, ImportSources } from "../../shared/shell";

interface Props {
  state: ShellState | null;
  onClose: () => void;
}

const ITEMS: { value: ImportItem; label: string; disabled?: boolean }[] = [
  { value: "bookmarks", label: "书签" },
  { value: "history", label: "历史" },
  // Cookie 的导入不在本需求（移到后续 change），先不让选
  { value: "cookies", label: "Cookie（暂不支持）", disabled: true },
];

const STATUS_TEXT: Record<ImportProgressView["status"], string> = {
  running: "导入中…",
  done: "导入完成",
  partial: "部分完成",
  failed: "导入失败",
};

/** 每次「开始导入」一个新的 requestId：只认这次导入的进度。 */
let lastRequestId = 0;

export default function ImportWizard(props: Props): JSX.Element {
  const [sources, setSources] = createSignal<ImportSources>({ chrome: [], arc: [] });
  const [source, setSource] = createSignal<ImportSource>("chrome");
  const [profile, setProfile] = createSignal("");
  const [spaceId, setSpaceId] = createSignal(props.state?.activeSpaceId ?? "");
  const [items, setItems] = createSignal<ImportItem[]>(["bookmarks", "history"]);
  const [requestId, setRequestId] = createSignal<number | undefined>();
  const [progress, setProgress] = createSignal<ImportProgressView | undefined>();

  const profiles = () => sources()[source()];
  const finished = () => {
    const status = progress()?.status;
    return status !== undefined && status !== "running";
  };
  const canStart = () => profile() !== "" && spaceId() !== "" && items().length > 0;

  /** 换来源时默认选它的第一个 profile。 */
  const pickSource = (next: ImportSource) => {
    setSource(next);
    setProfile(sources()[next][0] ?? "");
  };

  onMount(() => {
    // 先订阅再请求列表：进度只可能在点「开始导入」之后到达，这里只是保持与其他订阅一致
    const off = window.alter.onImportProgress((p) => {
      if (p.requestId === requestId()) setProgress(p);
    });
    void window.alter.importSources().then((s) => {
      setSources(s);
      pickSource(source());
    });
    onCleanup(off);
  });

  const toggle = (item: ImportItem, on: boolean) =>
    setItems((list) => (on ? [...list.filter((i) => i !== item), item] : list.filter((i) => i !== item)));

  const start = () => {
    const id = ++lastRequestId;
    setRequestId(id);
    setProgress(undefined);
    window.alter.startImport({ requestId: id, source: source(), profile: profile(), spaceId: spaceId(), items: items() });
  };

  return (
    <section class="import-wizard" data-testid="import-wizard">
      <h3>导入</h3>
      <label>
        来源
        <select
          data-testid="import-source"
          disabled={requestId() !== undefined}
          onChange={(e) => pickSource(e.currentTarget.value as ImportSource)}
        >
          <option value="chrome" selected={source() === "chrome"}>
            Chrome
          </option>
          <option value="arc" selected={source() === "arc"}>
            Arc
          </option>
        </select>
      </label>
      <label>
        Profile
        <select
          data-testid="import-profile"
          disabled={requestId() !== undefined}
          onChange={(e) => setProfile(e.currentTarget.value)}
        >
          <For each={profiles()}>
            {(name) => (
              <option value={name} selected={name === profile()}>
                {name}
              </option>
            )}
          </For>
        </select>
      </label>
      <Show when={profiles().length === 0}>
        <p class="hint">没有找到可导入的 profile</p>
      </Show>
      <label>
        目标 Space
        <select
          data-testid="import-space"
          disabled={requestId() !== undefined}
          onChange={(e) => setSpaceId(e.currentTarget.value)}
        >
          <For each={props.state?.spaces ?? []}>
            {(space) => (
              <option value={space.id} selected={space.id === spaceId()}>
                {space.name}
              </option>
            )}
          </For>
        </select>
      </label>
      <fieldset>
        <For each={ITEMS}>
          {(item) => (
            <label class="check">
              <input
                type="checkbox"
                data-testid="import-item"
                value={item.value}
                checked={items().includes(item.value)}
                disabled={item.disabled || requestId() !== undefined}
                onChange={(e) => toggle(item.value, e.currentTarget.checked)}
              />
              {item.label}
            </label>
          )}
        </For>
      </fieldset>
      <Show when={requestId() === undefined}>
        <div class="actions">
          <button data-testid="import-start" class="primary" disabled={!canStart()} onClick={start}>
            开始导入
          </button>
          <button data-testid="import-cancel" onClick={() => props.onClose()}>
            取消
          </button>
        </div>
      </Show>
      <Show when={requestId() !== undefined}>
        <div class="result" data-testid="import-result">
          <p>{STATUS_TEXT[progress()?.status ?? "running"]}</p>
          <Show when={progress()}>
            {(p) => (
              <>
                <span>书签 {p().counts.bookmarks}</span>
                <span>历史 {p().counts.history}</span>
                <span>Cookie {p().counts.cookies}</span>
                <span>跳过 {p().counts.skipped}</span>
                <Show when={p().error}>
                  <p class="error">{p().error}</p>
                </Show>
                <Show when={p().warning}>
                  <p class="hint">{p().warning}</p>
                </Show>
              </>
            )}
          </Show>
        </div>
      </Show>
      <Show when={finished()}>
        <button data-testid="import-done" class="primary" onClick={() => props.onClose()}>
          完成
        </button>
      </Show>
    </section>
  );
}
