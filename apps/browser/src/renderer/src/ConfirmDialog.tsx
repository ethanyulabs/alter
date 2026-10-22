// 确认框：画在侧边栏里（内容区被标签页面的视图覆盖），用于删除 Space 这类不可撤销的操作
import type { JSX } from "solid-js";

interface Props {
  message: string;
  okLabel: string;
  onOk: () => void;
  onCancel: () => void;
}

export default function ConfirmDialog(props: Props): JSX.Element {
  return (
    <div class="confirm" role="alertdialog" aria-label={props.message}>
      <p>{props.message}</p>
      <div class="actions">
        <button data-testid="confirm-cancel" onClick={() => props.onCancel()}>
          取消
        </button>
        <button data-testid="confirm-ok" class="danger" onClick={() => props.onOk()}>
          {props.okLabel}
        </button>
      </div>
    </div>
  );
}
