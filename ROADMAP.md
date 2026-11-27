# ROADMAP

现在：**v0.1.0 计划**（这一版只有计划和骨架，没有可用的接口）。下一步 v0.2.0：取正文 + `web_extract` + `web_find` + 工具包。**v0.2.0 的 Phase 计划已写，等站主看过再开工。**

分工见 `CLAUDE.md`「二」和 `docs/decisions/0001`：云端 Claude 做标「云端」的 Phase（在 `cloud/v0.2` 分支上），本机做标「本机」的；本机 Claude 审核、合并、commit、打 tag。

## 版本表

| 版本 | 内容 | 谁做 | 状态 |
| --- | --- | --- | --- |
| v0.1.0 | 计划、设计稿、骨架 | 本机 | 完成（没打 tag） |
| v0.2.0 | **取正文**：字节 → Markdown 正文（表格、链接、代码块不丢）、标题作者日期站名、编码识别、类型判断、「可能要开 JavaScript」提示、`article` / `full` 两种模式；`web_extract`、`web_find`；工具包和命令行 | 云端为主 | 计划已写，等批 |
| v0.3.0 | **抓取和搜索**：`web_fetch`（跳转、超时、大小上限、内网检查）；Tavily 的搜索、取页、爬站、站内地图（`web_search`、`web_crawl`、`web_map`）；报价和 `Usage`；模拟服务 | 云端为主，真跑在本机 | 大纲 |
| v0.4.0 | 接 Peck：`harness/tools/web.rs` 的抓取和取正文、`gateway/search.rs` 的 Tavily | 本机（Peck 仓库） | 大纲 |
| v0.5.0 | 站点规则（知网文献页、arXiv 摘要页、维基百科、GitHub 的 README 和 issue）、文章里的图、多页文章合并 | 云端为主，真实页核对在本机 | 大纲 |
| v0.6.0 | 接 Pomelo：`youzi/files.rs::fetch_page`、`youzi/office.rs::web_article`、`youzi/skills/web.rs`、`pub2md/search.rs` | 本机（Pomelo 仓库） | 大纲 |
| v0.7.0 | 接 Pith：`research/download.rs` 的抓取、`research/content.rs` 的网页取字、`research/tavily.rs` | 本机（Pith 仓库） | 大纲 |
| v1.0.0 | 完成体：冻结接口和工具清单，三家钉 v1.0.0，全量回归 | 本机 | 大纲 |

每一版都是「新增功能」：分 Phase，只在最后一个 Phase 收尾时打 tag。App 接入是 App 自己的一个版本，程度到时问站主；同一个 App 一次只接一个引擎。

## v0.2.0 取正文（计划）

设计见 `docs/design.md` §1、§2、§3（编码和类型）、§4、§6、§8。

| Phase | 谁 | 做什么 | 做完的样子 |
| --- | --- | --- | --- |
| 1 骨架与样本 | 云端 | `Error` 枚举（§1 的种类，中文 `Display`）；`Article`、`Block`、`Kind` 等类型；`tests/fixtures/html/` 的合成样本，每个带期望（标题、要包含的句子、不能包含的导航文字）；`examples/pages`（读 `local/pages/`，打印每页的标题、字数、开头几行、`needs_js`、段数，写进 `local/out/`）；依赖登记 `THIRD_PARTY.md` | 样本齐；`cargo test` 能读样本、断言期望（实现还是空的就标 `#[ignore]`）；`examples/pages` 在没有 `local/pages/` 时说「没有真实网页」 |
| 2 取正文 | 云端 | 编码（BOM → HTTP 头 → meta → UTF-8 → GB18030，试过样本再定要不要 `chardetng`）；类型判断（搬 Pith 的 `sniff`）；`article` 模式（搬 Peck 的 `html_to_markdown`、`tidy_markdown`、`needs_js`）；`full` 模式（Pith 的 `html_text` 的做法：全部文字、表格一行一行）；元数据；`blocks` 分段；Peck `web.rs` 和 Pith `content.rs` 里关于 HTML、编码、类型的测试搬过来 | 合成样本全过；搬来的测试全过；GBK、Big5 样本不乱码；PDF、PNG、ZIP 都认成「不是网页」；两种模式各有表格、链接、代码块不丢的用例 |
| 3 真实网页核对 | 本机 | 站主给网址清单，本机 Claude 抓快照到 `local/pages/`；跑 `examples/pages`，人看结果：把导航当正文、丢表格链接、乱码的，回到云端分支补规则和合成样本 | 核对记录写进 `HANDOFF.md`（多少页、哪些有问题、怎么改的）；问题都有对应的合成样本 |
| 4 工具包与 find | 云端（**等 engine-core v0.2.0 的 tag**） | 依赖 engine-core；`tools/web_extract.json`、`tools/web_find.json`（照 design §1，带例子）；`Toolkit` 实现；`find`（`Loc`：标题路径 + 第几段，`under` 范围）；命令行 `engine-web`（engine-core 的骨架）；`docs/toolkit.md` 补完 | 清单过 engine-core 的自检，例子都跑通；命令行 `tools`、`describe`、`call web_extract` / `web_find` 在样本上出对的 JSON；find 在样本上整句、按词、简繁、全角都能命中 |
| 5 收尾 | 本机 | 合并 `cloud/v0.2`；在 Mac 上跑 `./scripts/check.sh` 和真实网页再过一遍；README、`CHANGELOG.md`、`HANDOFF.md` | `./scripts/check.sh` 过；打 tag `v0.2.0` |

Phase 1–3 不依赖 engine-core，可以先做；Phase 4 等 engine-core 发 `v0.2.0`。engine-core 晚了就先做 Phase 3，或先把 v0.3 的抓取开个头（另开 `cloud/v0.3`，等 v0.2 发完再合）。

## 各 App 接入进度

| App | 接哪一版 | 状态 |
| --- | --- | --- |
| Peck | v0.4.0 | 没开始 |
| Pomelo | v0.6.0 | 没开始 |
| Pith | v0.7.0 | 没开始 |

## 用户验收清单

要站主亲自看、亲自做的，不挡开发：

- [ ] v0.2.0：在终端里对几份常看的网页跑 `engine-web call web_extract`，正文读起来对不对、没有把导航和广告当正文；跑一次 `web_find` 找一句话。
- [ ] v0.3.0：`engine-web call web_search` 搜一个问题，结果和花费对；爬站不加 `--yes` 只报价。
- [ ] v0.4.0：Peck 正式版里让模型搜一次、读一页，引用的网址对；内网地址被拒。
