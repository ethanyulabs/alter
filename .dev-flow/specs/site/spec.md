# site Specification

## Purpose
alter 的公开主页，介绍产品并给出安装方式。

## Requirements

### Requirement: 主页给出安装方式
系统 SHALL 在 `site/index.html`（英文）与 `site/zh/index.html`（中文）中各给出安装命令、Release 下载页链接、仅支持 Apple Silicon 的说明、浏览器直接下载 DMG 被 Gatekeeper 拦下时的处理办法、`alter skill install` 的用法，以及到另一语言页面的链接。

#### Scenario: 英文主页
- WHEN 读取 `site/index.html`
- THEN 其中有一个 `<code>` 元素的文本恰好为安装命令
- AND 文本包含 `Apple Silicon`、`System Settings → Privacy & Security → Open Anyway` 与 `alter skill install`
- AND 有 `href` 为 `https://github.com/ethanyulabs/alter/releases/latest` 的链接和 `href` 为 `zh/` 的链接

#### Scenario: 中文主页
- WHEN 读取 `site/zh/index.html`
- THEN 其中有一个 `<code>` 元素的文本恰好为安装命令，`<html>` 的 `lang` 为 `zh-CN`
- AND 文本包含 `Apple Silicon`、`系统设置 → 隐私与安全性 → 仍要打开` 与 `alter skill install`
- AND 有 `href` 为 `../` 的链接

### Requirement: 站点自包含
系统 SHALL 让 `site/` 下的页面只引用 `site/` 内存在的文件作为脚本、样式与图片，不从任何外部地址加载资源。

#### Scenario: 相对引用都能解析
- WHEN 遍历 `site/` 下每个 `.html` 文件中所有 `src` 属性与 `<link>` 的 `href`
- THEN 每个值都是相对路径，按该文件所在目录解析后对应 `site/` 下一个存在的文件
- AND 没有一个值以 `http:`、`https:` 或 `//` 开头

### Requirement: 各处的安装命令一致
系统 SHALL 让主页、中文页、README 与 `scripts/install.sh` 头部用法注释中的安装命令逐字相同。

#### Scenario: 四处相同
- WHEN 从 `site/index.html`、`site/zh/index.html`、`README.md` 与 `scripts/install.sh` 中取出包含 `releases/latest/download/install.sh` 的那条命令
- THEN 四处取出的命令都恰好为安装命令
