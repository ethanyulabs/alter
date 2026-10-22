// SDK 的入口：一组共享同一个 Gateway 连接的函数；runtime.ts 把默认实例挂到全局
import type { TakeOverResult, TaskSpaceInfo, TaskSpaceSummary } from "@alter/protocol";

import { Client, defaultGatewayFile } from "./client";
import { TaskSpace } from "./task-space";

export interface AlterOptions {
  /** gateway.json 的路径；默认 `ALTER_GATEWAY_FILE` 或 `<ALTER_HOME>/gateway.json` */
  gatewayFile?: string;
}

export function createAlter(opts: AlterOptions = {}) {
  const client = new Client(opts.gatewayFile ?? defaultGatewayFile());
  return {
    /** 按名称新建 TaskSpace，或按数字 id 恢复已有的。 */
    async taskSpace(nameOrId: string | number, opts: { spaceId?: string } = {}): Promise<TaskSpace> {
      const info =
        typeof nameOrId === "number"
          ? await client.call<TaskSpaceInfo>("taskSpace.resume", { id: nameOrId })
          : await client.call<TaskSpaceInfo>("taskSpace.create", { name: nameOrId, ...opts });
      return new TaskSpace(client, info);
    },
    /** 用户交还后接回控制权；用户尚未交还时抛 `E_USER_CONTROL`。返回的 TaskSpace 的 `userPage()` 是用户最后所在的页面。 */
    async takeOverTaskSpace(id: number): Promise<TaskSpace> {
      const r = await client.call<TakeOverResult>("taskSpace.takeOver", { id });
      const t = new TaskSpace(client, await client.call<TaskSpaceInfo>("taskSpace.resume", { id }));
      t.setUserPage(r.userPage);
      return t;
    },
    /** 认领一个已有的 TaskSpace（与按 id 恢复相同）。 */
    async claimTaskSpace(id: number): Promise<TaskSpace> {
      return new TaskSpace(client, await client.call<TaskSpaceInfo>("taskSpace.resume", { id }));
    },
    async listTaskSpaces(): Promise<TaskSpaceSummary[]> {
      return (await client.call<{ taskSpaces: TaskSpaceSummary[] }>("taskSpace.list")).taskSpaces;
    },
    /** 断开与 alterd 的连接。 */
    close(): void {
      client.close();
    },
  };
}

export type Alter = ReturnType<typeof createAlter>;
