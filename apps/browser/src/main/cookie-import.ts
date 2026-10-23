// 导入的 Cookie：alterd 解密后经 `shell.importCookies` 交来，写进目标 Space 的 partition。
// 值是用户的登录凭据，这里不打印任何 Cookie 内容。
import { session } from "electron";

import type { ImportedCookie } from "@alter/protocol";

import { partitionFor } from "./tab-views";

/** 把一批 Cookie 写进 `spaceId` 的 partition，返回写入成功与被 Chromium 拒绝的条数。 */
export async function importCookies(
  spaceId: string,
  cookies: ImportedCookie[],
): Promise<{ imported: number; rejected: number }> {
  const store = session.fromPartition(partitionFor(spaceId)).cookies;
  const results = await Promise.allSettled(cookies.map((c) => store.set(setDetails(c))));
  const imported = results.filter((r) => r.status === "fulfilled").length;
  return { imported, rejected: results.length - imported };
}

/**
 * `cookies.set` 的参数。源库的 `host_key` 不以点开头时是 host-only Cookie：不传 `domain`，
 * 由 url 的主机名决定（Electron 会给传入的 domain 补前导点，变成对子域名也生效；`__Host-` 前缀的还会被拒绝）。
 */
function setDetails(c: ImportedCookie): Electron.CookiesSetDetails {
  return c.domain.startsWith(".") ? c : { ...c, domain: undefined };
}
