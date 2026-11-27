# engine-web

Peck、Pith、Pomelo 共用的网页引擎（Rust 库）：搜索、抓取、取正文一起管。App 交进网址、字节或搜索词，拿回标题、Markdown 正文、链接和元数据，或者搜索结果和花费；怎么联网、要不要登录、结果存哪归各 App。

> 现在是 **v0.1.0**：只有计划和骨架，还没有可用的接口。进度见 `ROADMAP.md`，交接见 `HANDOFF.md`，设计见 `docs/design.md`。

## 计划的能力

- **取正文**（v0.2）：网页字节 → Markdown 正文（表格、链接、代码块不丢）、标题作者日期站名、编码识别（GBK 等不乱码）、类型判断、「可能要开 JavaScript」的提示；抽正文和全部文字两种模式。
- **抓取和搜索**（v0.3）：网址 → 内容，带跳转、超时、大小上限，默认不抓本机和内网；Tavily 的搜索、取页、爬站、站内地图，交回点数和花费，爬站先报价。
- **站点规则**（v0.5）：知网文献页、arXiv 摘要页、维基百科、GitHub 的 README 和 issue；文章里的图；多页文章合并。
- **给 Agent 的工具包**：`web_search`、`web_fetch`、`web_extract`、`web_crawl`、`web_map`、`web_find`，一份工具清单出 Rust 接口、App 里的工具和命令行（格式来自 engine-core）。

## App 怎么引用

钉住一个 tag：

```toml
[dependencies]
engine-web = { git = "https://github.com/elenskye/engine-web", tag = "v0.x.y" }
```

升级改 tag，回退改回上一个 tag。每版接口变化见 `CHANGELOG.md`。

## 开发

```bash
./scripts/check.sh
```

规则见 `CLAUDE.md`（Codex 读 `AGENTS.md`，是同一份）。
