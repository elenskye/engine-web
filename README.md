# engine-web

Peck、Pith、Pomelo 共用的网页引擎（Rust 库）：搜索、抓取、取正文一起管。App 交进网址、字节或搜索词，拿回标题、Markdown 正文、链接和元数据，或者搜索结果和花费；怎么联网、要不要登录、结果存哪归各 App。

> 现在在做 **v0.2.0**（取正文）：Phase 1–2 在云端分支 `cloud/v0.2` 上做完，等本机审核和真实网页核对，还没发版。进度见 `ROADMAP.md`，交接见 `HANDOFF.md`，设计见 `docs/design.md`。

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

## 取正文（v0.2）

```rust
use engine_web::{extract, ExtractOptions, Mode};

let article = extract(&bytes, Some("text/html; charset=gbk"), "https://example.com/a.html", &ExtractOptions::default())?;
article.title;      // 标题
article.markdown;   // Markdown 正文（article 模式）或全部文字（Mode::Full）
article.blocks;     // 分段：Block { heading_path, paragraph, kind, text }，block.label() 是「〈引言 › 方法〉第 3 段」
article.needs_js;   // 正文很短、脚本很多：可能要开 JavaScript
```

不是网页（PDF、图片、Office、压缩包、音视频）交 `Error::NotWeb(Binary)`，App 再交给别的引擎。单独用的：`sniff`（类型）、`decode`（编码）。

## 开发

```bash
./scripts/check.sh
```

```bash
cargo run --example pages -- tests/fixtures/html
```

`examples/pages` 对一个目录里的每份网页跑两种模式，打印标题、字数、段数、开头几行，结果写进 `local/out/`（不带参数时读 `local/pages/` 的真实网页快照，快照用 `scripts/snapshot.sh` 抓）。合成样本在 `tests/fixtures/html/`，写法见那里的 `README.md`；不是 UTF-8 的和二进制的样本用 `cargo run --example make_samples` 生成。

规则见 `CLAUDE.md`（Codex 读 `AGENTS.md`，是同一份）。
