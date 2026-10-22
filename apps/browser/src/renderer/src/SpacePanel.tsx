// Space 编辑面板：新建时填名称、选颜色后「创建」；编辑时可「保存」或「删除」（先确认）
import { SPACE_COLORS, type SpaceView } from "@alter/protocol";
import { createSignal, For, Show, type JSX } from "solid-js";

import ConfirmDialog from "./ConfirmDialog";

/** 名称规则与 alterd 一致：去掉首尾空白后 1–32 个字符。 */
const MAX_NAME = 32;

interface Props {
  /** 不给时是新建 */
  space?: SpaceView;
  /** 是否还有别的 Space（最后一个 Space 不能删） */
  canDelete: boolean;
  onClose: () => void;
}

export default function SpacePanel(props: Props): JSX.Element {
  const [name, setName] = createSignal(props.space?.name ?? "");
  const [color, setColor] = createSignal<string>(props.space?.color ?? SPACE_COLORS[0]);
  const [confirming, setConfirming] = createSignal(false);

  const valid = () => {
    const n = [...name().trim()].length;
    return n >= 1 && n <= MAX_NAME;
  };

  const save = () => {
    if (!valid()) return;
    const space = props.space;
    if (space) window.alter.updateSpace({ spaceId: space.id, name: name(), color: color() });
    else window.alter.createSpace({ name: name(), color: color() });
    props.onClose();
  };

  return (
    <div
      class="space-panel"
      onKeyDown={(e) => {
        if (e.key === "Escape") props.onClose();
      }}
    >
      <input
        data-testid="space-name-input"
        placeholder="Space 名称"
        maxLength={64}
        value={name()}
        onInput={(e) => setName(e.currentTarget.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") save();
        }}
        ref={(el) => queueMicrotask(() => el.focus())}
      />
      <div class="colors">
        <For each={SPACE_COLORS}>
          {(c) => (
            <button
              data-testid="space-color"
              data-color={c}
              classList={{ selected: color() === c }}
              style={{ background: c }}
              aria-label={c}
              onClick={() => setColor(c)}
            />
          )}
        </For>
      </div>
      <div class="actions">
        <Show when={props.space}>
          <button data-testid="space-delete" class="danger" disabled={!props.canDelete} onClick={() => setConfirming(true)}>
            删除
          </button>
        </Show>
        <span class="spacer" />
        <button data-testid="space-cancel" onClick={() => props.onClose()}>
          取消
        </button>
        <button data-testid="space-save" class="primary" disabled={!valid()} onClick={save}>
          {props.space ? "保存" : "创建"}
        </button>
      </div>
      <Show when={confirming() && props.space}>
        {(space) => (
          <ConfirmDialog
            message={`删除 Space「${space().name}」？其中的标签和 Cookie 都会被清除，不能恢复。`}
            okLabel="删除"
            onCancel={() => setConfirming(false)}
            onOk={() => {
              window.alter.deleteSpace(space().id);
              props.onClose();
            }}
          />
        )}
      </Show>
    </div>
  );
}
