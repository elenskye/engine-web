# engine-web

Peck、Pith、Pomelo 共用的网页引擎（Rust 库）：搜索、抓取、取正文一起管。App 交进网址、字节或搜索词，拿回标题、Markdown 正文、链接和元数据，或者搜索结果和花费；怎么联网、要不要登录、结果存哪归各 App。

> 现在是 **v0.3.0**（tag，2026-11-27）：取正文、抓取、Tavily 搜索，六个工具 + 命令行。接口和 App 怎么接见 `CHANGELOG.md`，进度见 `ROADMAP.md`，设计见 `docs/design.md`。

## 能力

- **取正文**（v0.2，已做）：网页字节 → Markdown 正文（表格、链接、代码块不丢）、标题作者日期站名、编码识别（GBK、Big5、Shift_JIS 等不乱码）、类型判断（PDF、图片、Office 等认出来交回 App）、「可能要开 JavaScript」的提示；抽正文和全部文字两种模式；`find` 在网页里找字。
- **抓取和搜索**（v0.3，已做）：网址 → 内容，自己跟同一个网站里的跳转，有超时和大小上限，解压 gzip / br，默认不抓本机和内网（钉住检查过的地址再连）；Tavily 的搜索、取页、爬站、站内地图，本地报价、按 Tavily 报的点数记花费，爬站和地图先问。
- **站点规则**（v0.5）：知网文献页、arXiv 摘要页、维基百科、GitHub 的 README 和 issue；文章里的图；多页文章合并。
- **给 Agent 的工具包**（已做）：`web_extract`、`web_find`、`web_fetch`、`web_search`、`web_crawl`、`web_map`。一份工具清单出 Rust 接口、App 里的工具和命令行（格式来自 engine-core）。

## App 怎么引用

钉住一个 tag：

```toml
[dependencies]
engine-web = { git = "https://github.com/elenskye/engine-web", tag = "v0.x.y" }
```

升级改 tag，回退改回上一个 tag。每版接口变化见 `CHANGELOG.md`。

## Rust 接口

```rust
use engine_web::{extract, fetch, ExtractOptions, FetchOptions, Ureq};
use engine_web::engine_core::Cancel;

let got = fetch("https://example.com/a.html", &FetchOptions::default(), &Ureq, &Cancel::new())?;  // 网址 → 字节
let article = extract(&got.bytes, got.content_type.as_deref(), &got.url, &ExtractOptions::default())?;
article.title;      // 标题
article.markdown;   // Markdown 正文（article 模式）或全部文字（Mode::Full）
article.blocks;     // 分段：Block { heading_path, paragraph, kind, text }，block.label() 是「〈引言 › 方法〉第 3 段」
article.needs_js;   // 正文很短而有脚本：可能要开 JavaScript
```

不是网页（PDF、图片、Office、压缩包、音视频）`extract` 交 `Error::NotWeb(Binary)`，App 再交给别的引擎。别的：`find`（在 `Article` 里找字）、`sniff`（类型）、`decode`（编码）、`tavily::Client`（搜索、取页、爬站、地图）、`tavily::quote`（本地报价）。

## 给 Agent 的工具包

`engine_web::tools::toolkit()`（默认设置）或 `toolkit_with(Web { … })`（App 的 UA、上限、`Transport`）是 engine-core 格式的工具包，六个工具。清单在 `tools/*.json`，App 怎么接、能覆盖什么见 `docs/toolkit.md`。命令行：

```bash
cargo run --bin engine-web -- tools
```

```bash
cargo run --bin engine-web -- call web_fetch '{"url":"https://www.rust-lang.org/"}'
```

```bash
cargo run --bin engine-web -- call web_find '{"file":"tests/fixtures/html/zh-news.html","query":"起步 2 元"}'
```

要 Tavily 的工具从环境变量 `ENGINE_KEY_TAVILY` 取密钥；花钱的不加 `--yes` 只报价。

## 开发

```bash
./scripts/check.sh
```

检查分两种配置跑（加不加 feature `fake`，模拟服务的测试要它）。真跑（联网、花 Tavily 点数）的测试在 `tests/real.rs`，标了 `#[ignore]`，做法见 `HANDOFF.md`「真跑」。

```bash
cargo run --example pages -- tests/fixtures/html
```

`examples/pages` 对一个目录里的每份网页跑两种模式，打印标题、字数、段数、开头几行，结果写进 `local/out/`（不带参数时读 `local/pages/` 的真实网页快照：`cp docs/real-pages.txt local/pages/urls.txt && ./scripts/snapshot.sh`）。合成样本在 `tests/fixtures/html/`，写法见那里的 `README.md`；不是 UTF-8 的和二进制的样本用 `cargo run --example make_samples` 生成。

规则见 `CLAUDE.md`（Codex 读 `AGENTS.md`，是同一份）。
