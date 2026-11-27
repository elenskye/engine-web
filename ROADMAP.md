# ROADMAP

现在：**v0.2.0 已发**（tag `v0.2.0`，2026-11-27）：取正文、`web_extract`、`web_find`、命令行。下一步 **v0.3.0 抓取和搜索**：Phase 计划还没写，本机写好、站主看过，云端再开 `cloud/v0.3`。

分工见 `CLAUDE.md`「二」和 `docs/decisions/0001`：云端 Claude 做标「云端」的 Phase（在 `cloud/v<主>.<次>` 分支上），本机做标「本机」的；本机 Claude 审核、合并、commit、打 tag。

## 版本表

| 版本 | 内容 | 谁做 | 状态 |
| --- | --- | --- | --- |
| v0.1.0 | 计划、设计稿、骨架 | 本机 | 完成（没打 tag） |
| v0.2.0 | **取正文**：字节 → Markdown 正文（表格、链接、代码块不丢）、标题作者日期站名、编码识别、类型判断、「可能要开 JavaScript」提示、`article` / `full` 两种模式；`web_extract`、`web_find`；工具包和命令行 | 云端为主 | **已发**（2026-11-27） |
| v0.3.0 | **抓取和搜索**：`web_fetch`（跳转、超时、大小上限、内网检查、解压 gzip / br）；Tavily 的搜索、取页、爬站、站内地图（`web_search`、`web_crawl`、`web_map`）；报价和 `Usage`；模拟服务 | 云端为主，真跑在本机 | 下一步，Phase 计划待写 |
| v0.4.0 | 接 Peck：`harness/tools/web.rs` 的抓取和取正文、`gateway/search.rs` 的 Tavily | 本机（Peck 仓库） | 大纲 |
| v0.5.0 | 站点规则（知网文献页、arXiv 摘要页、维基百科、GitHub 的 README 和 issue）、文章里的图、多页文章合并 | 云端为主，真实页核对在本机 | 大纲 |
| v0.6.0 | 接 Pomelo：`youzi/files.rs::fetch_page`、`youzi/office.rs::web_article`、`youzi/skills/web.rs`、`pub2md/search.rs` | 本机（Pomelo 仓库） | 大纲 |
| v0.7.0 | 接 Pith：`research/download.rs` 的抓取、`research/content.rs` 的网页取字、`research/tavily.rs` | 本机（Pith 仓库） | 大纲 |
| v1.0.0 | 完成体：冻结接口和工具清单，三家钉 v1.0.0，全量回归 | 本机 | 大纲 |

每一版都是「新增功能」：分 Phase，只在最后一个 Phase 收尾时打 tag。App 接入是 App 自己的一个版本，程度到时问站主；同一个 App 一次只接一个引擎。

## v0.2.0 取正文（已发）

| Phase | 谁 | 结果 |
| --- | --- | --- |
| 1 骨架与样本 | 云端 | `Error`、`Article` 等类型；合成样本带期望；`examples/pages` |
| 2 取正文 | 云端 | 编码（chardetng，`decisions/0005`）、类型判断、`article` / `full` 两种模式、元数据、分段；Peck、Pith 的测试搬进 `tests/ported.rs` |
| 3 真实网页核对 | 云端 + 本机 | 云端从海外抓 36 页、查出 11 处；本机在国内网络下补到 63 页，又查出 9 处。都改了规则、配了合成样本（网址在 `docs/real-pages.txt`） |
| 4 工具包与 find | 云端 | 钉 engine-core `v0.2.0`；两份工具清单、`find`、命令行、`docs/toolkit.md` |
| 5 收尾 | 本机 | 合并 `cloud/v0.2`；`src/html.rs` 拆成 `src/html/` 四个模块；Mac 上 `./scripts/check.sh` 过；打 tag |

## 各 App 接入进度

| App | 接哪一版 | 状态 |
| --- | --- | --- |
| Peck | v0.4.0 | 没开始 |
| Pomelo | v0.6.0 | 没开始 |
| Pith | v0.7.0 | 没开始 |

## 用户验收清单

要站主亲自看、亲自做的，不挡开发：

- [ ] v0.2.0：把几份常看的网页存下来（浏览器的「存储为…」，格式选网页源码，不要网页归档），在本仓库目录的终端里跑 `cargo run -q --bin engine-web -- call web_extract '{"file":"<路径>","url":"<网址>"}'`，看正文对不对、有没有把导航和广告当正文；再把 `web_extract` 换成 `web_find`、加 `"query":"<文章里的一句话>"`，看找到的段落对不对。本机已核对过的 63 页的结果在 `local/out/`（每页一个 `.article.md`），也可以直接翻。
- [ ] v0.3.0：`engine-web call web_search` 搜一个问题，结果和花费对；爬站不加 `--yes` 只报价。
- [ ] v0.4.0：Peck 正式版里让模型搜一次、读一页，引用的网址对；内网地址被拒。
