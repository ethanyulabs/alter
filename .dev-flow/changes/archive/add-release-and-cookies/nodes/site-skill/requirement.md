---
id: add-release-and-cookies
node: N3
title: 主页与 agent skill
repos: [alter]
revision: 1
approved_revision: 1
---

# N3 主页与 agent skill

<!-- 流程路线型需求中一个节点（子流程）的需求文档。这里写本节点的 Spec Delta；整体（背景、范围、非功能需求）在父级 requirement.md，节点顺序在 flow.md。保留字保持英文："Spec Delta:"、"ADDED/MODIFIED/REMOVED/RENAMED Requirement:"、"Scenario:"、WHEN/THEN/AND、SHALL/MUST。语法见 references/formats.md §3 和 §14 -->

## 范围
- 交付：
  - `site/` 静态主页：英文 `site/index.html`，中文 `site/zh/index.html`，共用 `site/style.css`。推送到 main 时由 GitHub Actions 发布到 GitHub Pages。
  - `skills/alter-browser/`：`SKILL.md` 与 `references/`。agent 读了它就能用 `alter nodejs` 完成网页任务、交接控制权、接入 Playwright。
  - README 的「安装」一节，以及去掉「工程骨架阶段」这一过时的现状描述。
- 依赖（来自 N2）：
  - install 命令：`curl -fsSL https://github.com/ethanyulabs/alter/releases/latest/download/install.sh | bash`。
  - `alter skill install` 命令。
  - 包内 skill 的路径：`Contents/Resources/skills/alter-browser`。本节点把 N2 的占位 `SKILL.md` 换成正式内容。
- 不做：
  - 自定义域名、站点统计、博客、文档站生成器。
  - skill 的多语言版本：skill 用英文写，与 ego-browser skill 保持一致，agent 读英文最稳。
  - 主页里的在线演示。

术语：
- **安装命令**：恰好为 `curl -fsSL https://github.com/ethanyulabs/alter/releases/latest/download/install.sh | bash` 的一行。
- **SDK 公开 API**：分三部分：
  - `packages/sdk/src/runtime.ts` 挂到全局的函数：`taskSpace`、`takeOverTaskSpace`、`claimTaskSpace`、`listTaskSpaces`。
  - `TaskSpace` 原型上的公开方法。
  - `Page` 原型上的公开方法，外加 `page.mouse.*` 与 `page.keyboard.*`。
  - 名字以 `_` 开头的不算公开 API。

## Spec Delta: site
alter 的公开主页，介绍产品并给出安装方式。

### ADDED Requirement: R1 主页给出安装方式
系统 SHALL 在 `site/index.html`（英文）与 `site/zh/index.html`（中文）中各给出安装命令、Release 下载页链接、仅支持 Apple Silicon 的说明、浏览器直接下载 DMG 被 Gatekeeper 拦下时的处理办法、`alter skill install` 的用法，以及到另一语言页面的链接。

#### Scenario: R1.S1 英文主页
- WHEN 读取 `site/index.html`
- THEN 其中有一个 `<code>` 元素的文本恰好为安装命令
- AND 文本包含 `Apple Silicon`、`System Settings → Privacy & Security → Open Anyway` 与 `alter skill install`
- AND 有 `href` 为 `https://github.com/ethanyulabs/alter/releases/latest` 的链接和 `href` 为 `zh/` 的链接

#### Scenario: R1.S2 中文主页
- WHEN 读取 `site/zh/index.html`
- THEN 其中有一个 `<code>` 元素的文本恰好为安装命令，`<html>` 的 `lang` 为 `zh-CN`
- AND 文本包含 `Apple Silicon`、`系统设置 → 隐私与安全性 → 仍要打开` 与 `alter skill install`
- AND 有 `href` 为 `../` 的链接

### ADDED Requirement: R2 站点自包含
系统 SHALL 让 `site/` 下的页面只引用 `site/` 内存在的文件作为脚本、样式与图片，不从任何外部地址加载资源。

#### Scenario: R2.S1 相对引用都能解析
- WHEN 遍历 `site/` 下每个 `.html` 文件中所有 `src` 属性与 `<link>` 的 `href`
- THEN 每个值都是相对路径，按该文件所在目录解析后对应 `site/` 下一个存在的文件
- AND 没有一个值以 `http:`、`https:` 或 `//` 开头

### ADDED Requirement: R3 各处的安装命令一致
系统 SHALL 让主页、中文页、README 与 `scripts/install.sh` 头部用法注释中的安装命令逐字相同。

#### Scenario: R3.S1 四处相同
- WHEN 从 `site/index.html`、`site/zh/index.html`、`README.md` 与 `scripts/install.sh` 中取出包含 `releases/latest/download/install.sh` 的那条命令
- THEN 四处取出的命令都恰好为安装命令

## Spec Delta: agent-skill
给 AI agent 的 alter 使用说明，随 app 打包并可用 `alter skill install` 安装。

### ADDED Requirement: R4 SKILL.md 的入口信息
系统 SHALL 提供 `skills/alter-browser/SKILL.md`，其 front matter 有 `name: alter-browser` 与一段说明何时使用的 `description`，正文给出用 `alter nodejs` heredoc 运行脚本的最小示例，并链接 `references/` 下的 `api.md`、`handoff.md`、`playwright.md` 与 `install.md`。

#### Scenario: R4.S1 front matter 与链接
- WHEN 读取 `skills/alter-browser/SKILL.md`
- THEN front matter 能被解析，`name` 为 `alter-browser`，`description` 非空且包含 `alter`
- AND 正文第一个 `bash` 代码块包含 `alter nodejs <<'EOF'`
- AND 正文链接的 `references/api.md`、`references/handoff.md`、`references/playwright.md`、`references/install.md` 都存在

### ADDED Requirement: R5 API 参考与 SDK 一致
系统 SHALL 让 `references/api.md` 为每个 SDK 公开 API 各有一个以其调用名开头的三级标题（例如 `### page.click`），且不出现 SDK 里不存在的调用名。

#### Scenario: R5.S1 文档与导出双向一致
- WHEN 用 SDK 的 `Page`、`TaskSpace` 原型与 `runtime.ts` 挂到全局的函数求出公开 API 的调用名集合，再从 `references/api.md` 的三级标题中取出调用名集合
- THEN 两个集合相等

#### Scenario: R5.S2 SDK 新增方法而文档没写时失败
- WHEN 测试在 `Page` 原型上临时加一个方法 `zzzProbe` 后重新求公开 API 集合
- THEN 两个集合不相等，差集恰好是 `page.zzzProbe`

### ADDED Requirement: R6 交接控制权的指引
系统 SHALL 在 `references/handoff.md` 中说明：收到 `E_USER_CONTROL` 时不得重试、要等用户交还；遇到登录、验证码时用 `handOff()` 交给用户，再用 `waitForControl()` 等待。

#### Scenario: R6.S1 关键规则写明
- WHEN 读取 `skills/alter-browser/references/handoff.md`
- THEN 文本包含 `E_USER_CONTROL`、`do not retry`、`handOff()` 与 `waitForControl()`

### ADDED Requirement: R7 示例可以真实运行
系统 SHALL 让 `SKILL.md` 第一个 `bash` 代码块中的示例在 alter 运行时可以原样执行成功（只把示例里的 `https://example.com` 换成可访问的地址）。

#### Scenario: R7.S1 对本地页面跑快速上手示例
- WHEN app 在运行，本地测试服务器提供标题为 `Example Domain` 的页面，把 `SKILL.md` 第一个 `bash` 代码块中的 `https://example.com` 替换为该页面地址后用 bash 执行
- THEN 退出码为 0，stdout 包含 `Example Domain`

## 变更记录
- 2026-10-22: 创建。
