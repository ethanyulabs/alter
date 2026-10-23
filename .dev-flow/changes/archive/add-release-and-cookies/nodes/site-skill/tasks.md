---
change: add-release-and-cookies
---

# 执行文档：主页与 agent skill

<!-- 执行文档。每个任务是一个薄的垂直切片，能由一个测试单独证明。
行语法：- [ ] T<n> [covers:R1.S1] [after:T<k>] [P] <做什么> — seam: <测试驱动的公开入口> — verify: <命令>
有变更的场景用 RED/GREEN，已有行为用 BASELINE，没有 covers 的任务用 GREEN/VERIFY。
对应的证据写进 log.md 之后才能勾选 [x]。执行时做不完的任务标 [blocked:<原因>]，问题写进 log.md。
完整语法见 references/formats.md -->

## Repo: alter

全节点约定：
- 测试命名：bun test 的 `describe` 或 `test` 标题以 `RC N3 R<n>.S<m>` 开头；e2e 标题同样如此。
- 测试文件：
  - `packages/sdk/tests/distribution.test.ts`：所有静态检查。
  - `apps/browser/e2e/skill.spec.ts`：真实运行。
- 仓库根目录用 `new URL("../../..", import.meta.url)` 求得。
- 安装命令只在测试里写一处常量：`const INSTALL = "curl -fsSL https://github.com/ethanyulabs/alter/releases/latest/download/install.sh | bash"`，其余地方都与它比较。
- 文案：
  - 英文页面与 skill 用英文。
  - 中文页面与 README 用中文，按仓库「文档用中文」的规则。
  - 代码注释用中文。
- 单向门：
  - 公开 API 集合的定义以 design.md「接口契约」为准。
  - 不改 SDK 的行为来迎合文档。文档与 SDK 不一致时改文档；如果确实是 SDK 缺方法，标为 blocked，不顺手加。

- [x] T1 [covers:R1.S1,R2.S1] 英文主页与共用样式 — seam: `distribution.test.ts` — verify: `bun test packages/sdk/tests/distribution.test.ts -t "RC N3 R1.S1|RC N3 R2"`
  - 新文件 `site/index.html`、`site/style.css`、`site/favicon.svg`，页面分节见 design.md「组件」。
  - Gatekeeper 的段落：从浏览器下载的 DMG 打开时，如果被拦，到 `System Settings → Privacy & Security → Open Anyway` 放行；用 install.sh 安装不会遇到这个问题。
  - 只允许内联的复制按钮脚本，放在 `<script>` 里，没有 `src`。字体用系统字体栈，不加载 Google Fonts，R2.S1 要求不从外部加载资源。
  - 支持深色模式：`prefers-color-scheme`。在手机宽度下不出现横向滚动；这一点人工看一眼，不写成场景。
  - 先写测试（RED：文件不存在），再写页面。

- [x] T2 [covers:R1.S2] [after:T1] 中文主页 — seam: `distribution.test.ts` — verify: `bun test packages/sdk/tests/distribution.test.ts -t "RC N3 R1.S2|RC N3 R2"`
  - `site/zh/index.html` 与英文页结构相同，引用 `../style.css`。R2.S1 的测试会自动覆盖这个新页面。

- [x] T3 [covers:R4.S1,R6.S1] skill 正文与 references — seam: `distribution.test.ts` — verify: `bun test packages/sdk/tests/distribution.test.ts -t "RC N3 R4|RC N3 R6"`
  - 替换 N2 的占位 `skills/alter-browser/SKILL.md`，新增 `references/{handoff,playwright,install}.md`。`api.md` 留给 T4。
  - 各节内容见 design.md「接口契约」。
  - 写 SDK 的签名与行为时，要先读 `packages/sdk/src/page.ts`、`task-space.ts`、`alter.ts` 和 `.dev-flow/infra/api.md`。不得写 SDK 没有的参数。
  - `waitForFileChooser` 还没有实现（推迟到后续 change），文档里要写明上传文件用 `page.setInputFiles`。
  - R4.S1 只检查链接到的四个文件存在，所以 T3 结束时 `api.md` 至少要有一个标题行占位。这样 R4.S1 在 T3 就能变绿，T4 再补全内容。
  - N2 的 `alter skill install` 测试用的是 `ALTER_SKILL_DIR` 指向的 fixture，不受本任务影响。`check-bundle.sh` 只检查 `SKILL.md` 存在，同样不受影响。

- [x] T4 [covers:R5.S1,R5.S2] [after:T3] `references/api.md` 与 SDK 公开 API 双向一致 — seam: `distribution.test.ts`（import `packages/sdk/src` 的 `Page`、`TaskSpace`；全局函数从 `runtime.ts` 读取） — verify: `bun test packages/sdk/tests/distribution.test.ts -t "RC N3 R5"`
  - 全局函数的取法：不 import `runtime.ts`，因为它会立即调用 `createAlter()` 去连网关。改为在测试里用正则匹配 `g["(\w+)"] = alter\.` 取出函数名，并在注释里写明这个前提。
  - `page.mouse` / `page.keyboard`：用 `new Page(假 client, …)` 构造实例后读取。构造参数仿照 `packages/sdk/tests/sdk.test.ts` 里已有的写法。
  - R5.S2：在 `try/finally` 里给 `Page.prototype.zzzProbe` 赋值，再 `delete`，免得污染同一进程里的其他测试。

- [x] T5 [covers:R3.S1] [after:T1,T2] README 的安装一节，以及四处安装命令一致 — seam: `distribution.test.ts` — verify: `bun test packages/sdk/tests/distribution.test.ts -t "RC N3 R3"`
  - README 的改动：
    - 新增「安装」一节，写安装命令、`alter skill install` 和卸载方法（删除 app 与 `~/.local/bin/alter`）。
    - 「现状」一节改成描述已有功能。
    - 「开发」一节加上 `bun run dist`。
  - `scripts/install.sh` 的头部注释里已有用法行（N2/T8 写的）。如果和常量不一致，改 install.sh 的注释，不改常量。

- [x] T6 [covers:R7.S1] [after:T3,T4] 真实执行 SKILL.md 的快速上手示例 — seam: Electron e2e（`apps/browser/e2e/skill.spec.ts`） — verify: `bun run e2e --grep "RC N3 R7"`
  - `e2e/fixtures/server.ts` 新增 `/example` 路由，`<title>Example Domain</title>`。
  - 用 `launchApp()` 启动 app，等 `gateway.json` 出现。从 `SKILL.md` 取出第一个 ```` ```bash ```` 代码块，把 `https://example.com` 换成本地服务器地址，然后以 `PATH=<repo>/target/debug:$PATH`、`ALTER_HOME=<该 app 的 home>` 执行 `bash -c`。
  - 断言 stdout 包含 `Example Domain`，退出码为 0。示例里调用了 `ts.finish()`，所以不会留下 TaskSpace。

- [x] T7 [after:T1,T2] GitHub Pages 发布工作流 — seam: `.github/workflows/pages.yml` — verify: `ruby -ryaml -e 'YAML.load_file(".github/workflows/pages.yml")' && bun run lint`
  - 字段见 design.md「组件」。工作流只能在 GitHub 上验证，对应父级 SC6；本地只确认 YAML 能被解析，记为 VERIFY。
  - 后续事项：维护者要在仓库 Settings → Pages 把 Source 设为 GitHub Actions。写进 log 的 `follow-up`，`dflow summary` 会把它带进 PR 描述。

- [x] T8 [after:T1,T2,T3,T4,T5,T6,T7] 节点门禁 — seam: 仓库门禁 — verify: `bun run ci:check && bun run e2e --grep "RC N3"`
  - 覆盖率：本节点只新增测试与文档，没有生产代码。`bun test --coverage` 只用来确认 `packages/sdk/src` 的覆盖率没有下降，在 log 里注明。

## Convergence

- [x] T9 [covers:R4.S1] 收敛第 1 轮：按 SDK 的实际行为改正 api.md 与 SKILL.md；front matter 用 Bun.YAML 解析；references 改为 Markdown 链接；去掉 metadata.version（CR1–CR9、CR11） — seam: `distribution.test.ts` — verify: `bun test packages/sdk/tests/distribution.test.ts && bun run e2e --grep "RC N3 R7"`
  - 测试：R4.S1 改用 `Bun.YAML.parse` 解析 front matter，并断言正文以 Markdown 链接 `](references/<文件>)` 指向四份 references（先 RED）。
  - api.md：
    - waitFor：成功时返回 true，超时抛 `E_TIMEOUT`；`selector` 只认原生 CSS；`ms` 不能超过 `timeoutMs`（默认 30 秒）。
    - listTaskSpaces：字段为 `{id, name, spaceId, ownership, status}`，包括已结束的。
    - screenshot：path 必须是绝对路径。
    - waitForEvent：超时抛普通 Error，默认 30 秒。
    - cdp：只要有任何 TaskSpace 在用户手里就回 `E_USER_CONTROL`。
  - SKILL.md：
    - 收到 E_USER_CONTROL 后先停下、告诉用户，再 `waitForControl()` 等用户交还，与 handoff.md 一致。
    - waitFor 的建议改为 CSS 选择器或 url。
    - 去掉 `metadata`；skill 的版本以 app 包为准，`alter skill install` 写入的 `.alter-skill` 已经记录了版本。
  - design.md：最小示例改为 `p1`；去掉 `metadata.version` 那一行，改写为「版本由 `.alter-skill` 记录」。
- [x] T10 [covers:R2.S1] 收紧 R2.S1：路径必须在 site/ 内；单双引号都识别；扫 CSS 里的 url() 与 @import（CR10） — seam: `distribution.test.ts` — verify: `bun test packages/sdk/tests/distribution.test.ts`
  - 用变异证明：临时在页面里加 `src='../README.md'`，或在 style.css 里加 `@import url(https://fonts.example/x.css)`，测试都应当失败。
- [x] T11 收敛整体第 1 轮：skill 写明看到 [alter:notice] 只转告用户、不要自己升级；文档补上 ~/Applications 的兜底位置（CR1、CR2） — seam: `distribution.test.ts` + 文档 — verify: `bun test packages/sdk/tests/distribution.test.ts`
  - SKILL.md 的 Rules 加一条：看到 stderr 里有 `[alter:notice] … run: alter upgrade` 时，把它转告用户；绝不要自己运行 `alter upgrade` 或 install.sh，因为那会退出用户正在用的 alter。install.md 里的 `alter upgrade` 也注明「由用户运行」。
  - 主页（英文和中文）、README、install.md：安装位置写明「/Applications，不可写时 ~/Applications」，卸载说明两个位置都列出。
  - distribution.test.ts 加一个守卫测试，不对应场景：SKILL.md 必须包含 `[alter:notice]` 和 `alter upgrade` 的禁令措辞；先 RED。
- [x] T12 收敛第 3 轮：错误表改为请用户重装；守卫测试绑定 runtime.ts 的提示前缀并禁止让 agent 自己重装或升级的措辞；design.md 同步 — seam: `distribution.test.ts` — verify: `bun test packages/sdk/tests/distribution.test.ts`
  - install.md：「找不到 bun」一行改为 Ask the user to reinstall alter。
  - 守卫测试：从 runtime.ts 取 `[alter:notice]` 所在的模板字符串前缀，断言 SKILL.md 含有它；断言 references 与 SKILL.md 里没有 `Reinstall with` 这类让 agent 自己重装的祈使句（先 RED：改 install.md 之前它应当失败）。
  - design.md：Rules 加「看到 [alter:notice] 只转告用户，不自己运行 alter upgrade 或安装脚本」，install.md 的描述加「只由用户升级或重装」。
