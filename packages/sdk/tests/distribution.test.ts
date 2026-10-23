// 随仓库发布的文档的契约：site/ 主页与 skills/alter-browser 与 SDK、安装方式保持一致。
// HTML 是本仓库手写的简单页面，用正则取 <code>、src、href、lang 就够，不引入 HTML 解析库。
import { describe, expect, test } from "bun:test";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

import { Page } from "../src/page";
import { TaskSpace } from "../src/task-space";

const ROOT = resolve(import.meta.dir, "../../..");
const SITE = join(ROOT, "site");
const SKILL = join(ROOT, "skills/alter-browser");
/** 安装命令：主页、中文页、README 与 install.sh 的用法注释都要逐字相同。 */
const INSTALL = "curl -fsSL https://github.com/ethanyulabs/alter/releases/latest/download/install.sh | bash";

function read(path: string): string {
  return readFileSync(path, "utf8");
}

/** HTML 实体还原成字符（页面里的 `|` 等写成实体时也能比较）。 */
function decode(s: string): string {
  return s
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&#x7c;|&#124;|&vert;/gi, "|");
}

/** 页面里所有 <code> 元素的文本。 */
function codes(html: string): string[] {
  return [...html.matchAll(/<code[^>]*>([\s\S]*?)<\/code>/g)].map((m) => decode(m[1]!.replace(/<[^>]+>/g, "")).trim());
}

/** 去掉标签后的可见文本（空白合并）。 */
function textOf(html: string): string {
  return decode(html.replace(/<script[\s\S]*?<\/script>|<style[\s\S]*?<\/style>/g, "").replace(/<[^>]+>/g, " "))
    .replace(/\s+/g, " ")
    .trim();
}

function hrefs(html: string): string[] {
  return [...html.matchAll(/<a\b[^>]*\bhref="([^"]*)"/g)].map((m) => m[1]!);
}

/** site/ 下全部 .html 文件。 */
function htmlFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) return htmlFiles(p);
    return name.endsWith(".html") ? [p] : [];
  });
}

describe("RC N3 site", () => {
  test("RC N3 R1.S1 英文主页给出安装方式", () => {
    const html = read(join(SITE, "index.html"));
    expect(codes(html)).toContain(INSTALL);
    const text = textOf(html);
    for (const want of ["Apple Silicon", "System Settings → Privacy & Security → Open Anyway", "alter skill install"]) {
      expect(text).toContain(want);
    }
    const links = hrefs(html);
    expect(links).toContain("https://github.com/ethanyulabs/alter/releases/latest");
    expect(links).toContain("zh/");
  });

  test("RC N3 R1.S2 中文主页给出安装方式", () => {
    const html = read(join(SITE, "zh/index.html"));
    expect(codes(html)).toContain(INSTALL);
    expect(html).toMatch(/<html[^>]*\blang="zh-CN"/);
    const text = textOf(html);
    for (const want of ["Apple Silicon", "系统设置 → 隐私与安全性 → 仍要打开", "alter skill install"]) {
      expect(text).toContain(want);
    }
    expect(hrefs(html)).toContain("../");
  });

  test("RC N3 R2.S1 相对引用都能解析，不从外部加载资源", () => {
    const files = htmlFiles(SITE);
    expect(files.length).toBeGreaterThan(0);
    const site = resolve(SITE);
    /** 一个引用：必须是相对路径，解析后落在 site/ 之内且文件存在（Pages 只发布 site/）。 */
    const check = (from: string, ref: string) => {
      expect(/^(https?:|\/\/|data:)/i.test(ref), `${from} 引用了外部资源 ${ref}`).toBe(false);
      const target = resolve(dirname(from), ref.split(/[?#]/)[0]!);
      expect(target === site || target.startsWith(site + "/"), `${from} 的 ${ref} 不在 site/ 里`).toBe(true);
      expect(existsSync(target), `${from} 的 ${ref} 不存在`).toBe(true);
    };
    const attr = (html: string, re: RegExp) => [...html.matchAll(re)].map((m) => m[2]!);
    for (const file of files) {
      const html = read(file);
      for (const ref of attr(html, /\bsrc=(["'])(.*?)\1/g)) check(file, ref);
      for (const tag of html.match(/<link\b[^>]*>/g) ?? []) {
        for (const ref of attr(tag, /\bhref=(["'])(.*?)\1/g)) check(file, ref);
      }
    }
    // 样式表里的 url() 与 @import 同样不能指向外部
    for (const css of readdirSync(SITE, { recursive: true }).map(String).filter((f) => f.endsWith(".css"))) {
      const file = join(SITE, css);
      const text = read(file);
      for (const m of text.matchAll(/url\(\s*(["']?)([^"')]+)\1\s*\)|@import\s+(["'])([^"']+)\3/g)) {
        check(file, (m[2] ?? m[4])!);
      }
    }
  });
});

/** SKILL.md 的 front matter（按 YAML 解析：agent 加载 skill 时也是这样读的）与正文。 */
function frontMatter(md: string): { meta: Record<string, unknown>; body: string } {
  const m = md.match(/^---\n([\s\S]*?)\n---\n([\s\S]*)$/);
  if (!m) throw new Error("SKILL.md 没有 front matter");
  return { meta: Bun.YAML.parse(m[1]!) as Record<string, unknown>, body: m[2]! };
}

/** Markdown 里第一个 ```bash 代码块的内容。 */
export function firstBashBlock(md: string): string {
  const m = md.match(/```bash\n([\s\S]*?)```/);
  if (!m) throw new Error("没有 bash 代码块");
  return m[1]!;
}

describe("RC N3 skill", () => {
  test("RC N3 R4.S1 front matter 与链接", () => {
    const { meta, body } = frontMatter(read(join(SKILL, "SKILL.md")));
    expect(meta["name"]).toBe("alter-browser");
    expect(typeof meta["description"]).toBe("string");
    expect((meta["description"] as string).length).toBeGreaterThan(0);
    expect(meta["description"]).toContain("alter");
    expect(firstBashBlock(body)).toContain("alter nodejs <<'EOF'");
    for (const ref of ["references/api.md", "references/handoff.md", "references/playwright.md", "references/install.md"]) {
      expect(body, `正文应当以 Markdown 链接指向 ${ref}`).toMatch(new RegExp(`\\]\\(${ref.replace(".", "\\.")}\\)`));
      expect(existsSync(join(SKILL, ref)), `${ref} 不存在`).toBe(true);
    }
  });

  test("RC N3 R6.S1 交接控制权的规则写明", () => {
    const text = read(join(SKILL, "references/handoff.md"));
    for (const want of ["E_USER_CONTROL", "do not retry", "handOff()", "waitForControl()"]) {
      expect(text).toContain(want);
    }
  });

  // 守卫（不对应场景）：SDK 会在 stderr 打出 `[alter:notice] … run: alter upgrade`，
  // agent 照做会让 install.sh 退出用户正在用的 alter；skill 必须写明只转告用户
  test("RC N3 看到更新提示时只转告用户，不自己升级", () => {
    const { body } = frontMatter(read(join(SKILL, "SKILL.md")));
    // SKILL.md 引用的提示必须就是 runtime.ts 实际打出的那句：模板里的 ${…} 对应文档里任意一段占位
    const printed = read(join(ROOT, "packages/sdk/src/runtime.ts")).match(/console\.error\(`(\[alter:notice\][^`]*)`/);
    expect(printed, "runtime.ts 里应当有 [alter:notice] 提示").not.toBeNull();
    const escape = (t: string) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const pattern = printed![1]!.split(/\$\{[^}]*\}/).map(escape).join("\\S+");
    expect(body, `SKILL.md 应当原样引用提示：${printed![1]}`).toMatch(new RegExp(pattern));
    expect(body).toMatch(/never run `alter upgrade`/i);
    expect(read(join(SKILL, "references/install.md"))).toMatch(/only the user runs `alter upgrade`/i);
    // 任何地方都不能让 agent 自己重装（「Reinstall with …」这类祈使句）
    for (const file of ["SKILL.md", ...readdirSync(join(SKILL, "references")).map((f) => `references/${f}`)]) {
      expect(read(join(SKILL, file)), `${file} 让 agent 自己重装或升级`).not.toMatch(/^\W*(\|.*\|\s*)?(reinstall|re-?run the installer)\b/im);
    }
  });
});

/** 源码里声明为 `private` 的成员名：运行时也在原型上，但不是公开 API。 */
function privateNames(source: string): Set<string> {
  return new Set([...source.matchAll(/^\s*private\s+(?:readonly\s+)?(?:async\s+)?(\w+)/gm)].map((m) => m[1]!));
}

/** 原型上的公开方法名（不含 constructor、`_` 开头与 private 的）。 */
function protoMethods(proto: object, source: string): string[] {
  const hidden = privateNames(source);
  return Object.getOwnPropertyNames(proto).filter(
    (n) => n !== "constructor" && !n.startsWith("_") && !hidden.has(n),
  );
}

/** SDK 公开 API 的调用名集合（定义见 N3 design.md「接口契约」）。 */
export function sdkApi(extra: { page?: string[] } = {}): Set<string> {
  const src = (f: string) => read(join(ROOT, "packages/sdk/src", f));
  const globals = [...src("runtime.ts").matchAll(/g\["(\w+)"\] = alter\./g)].map((m) => m[1]!);
  // mouse / keyboard 是实例属性：用一个不连网关的 Page 读出它们的方法名
  const page = new Page({} as never, 0, "p");
  return new Set([
    ...globals,
    ...protoMethods(TaskSpace.prototype, src("task-space.ts")).map((n) => `taskSpace.${n}`),
    ...[...protoMethods(Page.prototype, src("page.ts")), ...(extra.page ?? [])].map((n) => `page.${n}`),
    ...Object.keys(page.mouse).map((n) => `page.mouse.${n}`),
    ...Object.keys(page.keyboard).map((n) => `page.keyboard.${n}`),
  ]);
}

/** api.md 三级标题里的调用名（`(` 或空白之前的部分）。 */
function documented(): Set<string> {
  const md = read(join(SKILL, "references/api.md"));
  return new Set([...md.matchAll(/^### ([\w.]+)/gm)].map((m) => m[1]!));
}

function diff(a: Set<string>, b: Set<string>): string[] {
  return [...a].filter((x) => !b.has(x)).sort();
}

describe("RC N3 api", () => {
  test("RC N3 R5.S1 文档与 SDK 导出双向一致", () => {
    const api = sdkApi();
    const docs = documented();
    expect({ undocumented: diff(api, docs), unknown: diff(docs, api) }).toEqual({ undocumented: [], unknown: [] });
  });

  test("RC N3 R5.S2 SDK 新增方法而文档没写时失败", () => {
    const proto = Page.prototype as unknown as Record<string, unknown>;
    proto["zzzProbe"] = () => undefined;
    try {
      expect(diff(sdkApi(), documented())).toEqual(["page.zzzProbe"]);
    } finally {
      delete proto["zzzProbe"];
    }
  });
});

/** 文本里包含 install.sh 下载地址的那几行命令（去掉行首的注释符号与空白）。 */
function installLines(text: string): string[] {
  return text
    .split("\n")
    .filter((l) => l.includes("releases/latest/download/install.sh"))
    .map((l) => l.replace(/^\s*#\s*/, "").trim());
}

describe("RC N3 install command", () => {
  test("RC N3 R3.S1 主页、中文页、README 与 install.sh 用法注释里的安装命令逐字相同", () => {
    const fromHtml = (f: string) => codes(read(f)).filter((c) => c.includes("releases/latest/download/install.sh"));
    const sources: Record<string, string[]> = {
      "site/index.html": fromHtml(join(SITE, "index.html")),
      "site/zh/index.html": fromHtml(join(SITE, "zh/index.html")),
      "README.md": installLines(read(join(ROOT, "README.md"))),
      "scripts/install.sh": installLines(read(join(ROOT, "scripts/install.sh"))),
    };
    for (const [file, found] of Object.entries(sources)) {
      expect(found.length, `${file} 里没有安装命令`).toBeGreaterThan(0);
      for (const cmd of found) expect(cmd, file).toBe(INSTALL);
    }
  });
});
