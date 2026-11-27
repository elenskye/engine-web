# 0001 范围、各版本做到哪、分工和 Git

2026-11-27，照站主在引擎总仓库（`personal-engine-studio`，只在本机，不上 GitHub）定的事抄成本仓库自足的一份：云端 Claude 看不到总仓库，以这里为准。

## 范围

- **搜索、抓取、取正文一起管**（像 Claude Code 的搜索和抓取）：
  - 搜索接 Tavily 的 `/search`；
  - 抓取是网址 → 内容（跳转、超时、大小上限、编码、类型判断），本机取不到时可以改用 Tavily 的 `/extract`；
  - 爬站和站内地图接 Tavily 的 `/crawl`、`/map`；
  - 取正文是字节 → 标题、Markdown 正文、链接、作者日期站名、「可能要开 JavaScript」的提示。
- **不接**：DeepSeek 的搜索接口；模型自带的搜索（Claude、OpenAI、Gemini 走模型那一层，留在各 App）。
- **归 App**：
  - 怎么联网（代理、Cookie、User-Agent 交进来用）；
  - 要不要登录；
  - 结果存哪；
  - 要不要再让模型总结一遍；
  - WebKit 渲染脚本页。
- **钱和密钥**：照 engine-pdf 对 Mathpix 的规矩。
  - 密钥由 App 交进来，不进日志、错误信息和夹具。
  - 每次调用交回 `Usage`（Tavily 的点数和美元），单价由 App 给，默认每点 $0.008。
  - 搜索、取页默认不确认；爬站、地图按页数报价、默认要确认；App 可以覆盖。
- 真跑 Tavily 每版上限 $1。

## 各版本做到哪（站主在问卷里定的）

| 能力 | 第一版（第一个能接进 App 的版本） | 1.0 之前 |
| --- | --- | --- |
| 正文转 Markdown（表格、链接、代码块不丢） | ✓ | |
| 标题、作者、日期、站名 | ✓ | |
| 编码识别（GBK 这类老网页不乱码） | ✓ | |
| 「可能要开 JavaScript」的提示 | ✓ | |
| 认出不是网页的（PDF、图片、下载文件交回 App） | ✓ | |
| 文章里的图（地址和说明） | | ✓ |
| 多页文章合并 | | ✓ |

- 先写专门取法的网站：知网文献页、arXiv 摘要页、维基百科、GitHub 的 README 和 issue。每个站配真实网页样本；网站改版、规则失效时退回通用提取。
- 接入顺序：**Peck → Pomelo → Pith**。

## 所有引擎都守的

- Agent First：每项能力先定义成给模型用的工具；Rust 接口、App 里的 Agent 工具、命令行都从这一份清单出。
- 工具包和 find 的格式来自共用小库 engine-core（GitHub `elenskye/engine-core`，钉 tag）。
- 细则见 `CLAUDE.md`「一」到「五」。

## 分工和 Git

- **本机 Claude Code**：设计、计划、审核、合并、发版。
- **Codex**：在本机实施。
- **云端 Claude**（站主的另一个账号）：**只做本引擎里不涉及 Mac 专属的部分**。
  - 从 GitHub 上的 `main` 开 `cloud/v<主>.<次>` 分支（做 v0.2.0 用 `cloud/v0.2`），只推这一条分支。
  - 不碰 `main`、不打 tag、不用真实密钥花钱。
  - 本机 Claude `git fetch` 下来审核，在 Mac 上跑检查和真实网页核对，过了再并进 `main`（merge，保留原提交）。
- **提交说明**：`v<版本>: <description>`，英文一行，不署名。
- **push**：`main` 和 tag 只由站主推。
