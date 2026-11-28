# ROADMAP

现在：**v0.3.0 已发**（tag `v0.3.0`，2026-11-27）：取正文、抓取、Tavily 搜索，六个工具。正在做 **v0.4.0 站点规则、文章里的图、多页合并**（Phase 1、2 完成，下一步 Phase 3）。原 v0.4.0 接 Peck 和 v0.5.0 对调（2026-11-28 站主定：Peck 还在开发）。

分工见 `CLAUDE.md`「二」和 `docs/decisions/0001`：云端 Claude 做标「云端」的 Phase（在 `cloud/v<主>.<次>` 分支上），本机做标「本机」的；本机 Claude 审核、合并、commit、打 tag。

## 版本表

| 版本 | 内容 | 谁做 | 状态 |
| --- | --- | --- | --- |
| v0.1.0 | 计划、设计稿、骨架 | 本机 | 完成（没打 tag） |
| v0.2.0 | **取正文**：字节 → Markdown 正文（表格、链接、代码块不丢）、标题作者日期站名、编码识别、类型判断、「可能要开 JavaScript」提示、`article` / `full` 两种模式；`web_extract`、`web_find`；工具包和命令行 | 云端为主 | **已发**（2026-11-27） |
| v0.3.0 | **抓取和搜索**：`web_fetch`（跳转、超时、大小上限、内网检查、解压 gzip / br）；Tavily 的搜索、取页、爬站、站内地图（`web_search`、`web_crawl`、`web_map`）；报价和 `Usage`；模拟服务 | 本机（站主 2026-11-27 定） | **已发**（2026-11-27） |
| v0.4.0 | **站点规则、图、多页**：文献页（arXiv、期刊、工作论文和智库）、维基百科、GitHub 的 README 和 issue、微信公众号文章；文章里的图；多页文章自动合并（原 v0.5） | 本机（站主 2026-11-28 定） | 在做：Phase 1–2 完成 |
| v0.5.0 | 接 Peck：`harness/tools/web.rs` 的抓取和取正文、`gateway/search.rs` 的 Tavily（原 v0.4） | 本机（Peck 仓库） | 大纲 |
| v0.6.0 | 接 Pomelo：`youzi/files.rs::fetch_page`、`youzi/office.rs::web_article`、`youzi/skills/web.rs`、`pub2md/search.rs` | 本机（Pomelo 仓库） | 大纲 |
| v0.7.0 | 接 Pith：`research/download.rs` 的抓取、`research/content.rs` 的网页取字、`research/tavily.rs` | 本机（Pith 仓库） | 大纲 |
| v1.0.0 | 完成体：冻结接口和工具清单，三家钉 v1.0.0，全量回归 | 本机 | 大纲 |

每一版都是「新增功能」：分 Phase，只在最后一个 Phase 收尾时打 tag。App 接入是 App 自己的一个版本，程度到时问站主；同一个 App 一次只接一个引擎。

## v0.4.0 站点规则、文章里的图、多页合并（在做，本机）

全部在本机做（站主 2026-11-28 定）。不花钱：只用本机抓取，不真跑 Tavily。真实页只用国际上程序拿得到的，中国大陆有网络问题的网站不抓、不作样本（站主 2026-11-28 定）。

**范围**（站主 2026-11-28 定）：

| 类 | 哪些 | 交回什么 |
| --- | --- | --- |
| 文献页 | arXiv 摘要页；期刊文章页、工作论文和智库的文献页（按网页里通用的 `citation_*`、PRISM 元数据认，不按站名一个个写；PLOS、AEA、Nature 另有去噪声的规则）。知网、万方、维普、PubMed 移出（拿不到真实页，`decisions/0008`） | 题名、作者和机构、关键词、刊名卷期页、报告号、发表日期、DOI、编号、PDF 网址：写在工具结果的标题下面，另作结构化的 `paper` |
| 百科 | 维基百科（中英文） | 正文；信息框读成两列的表；引用角标留着、参考文献放最后；去掉「编辑」、红链的编辑网址、导航框 |
| 代码站 | GitHub 的 README（仓库首页、文件页）和 issue | README 正文，标题是「仓库名 / 文件名」；issue 的标题、状态、提问和每条回复（作者、时间） |
| 公众号 | 微信公众号文章 | 正文、公众号名、作者、发布时间、懒加载的图。它的正文默认藏起来、靠脚本显示，现在可能被当成隐藏元素删掉 |
| 图 | 所有网页 | 每张图的网址、说明、在哪一段（`images`）；正文照旧只留说明；模型要看哪张就用 `web_fetch` 取原件（已经能用） |
| 多页 | 同一网站的「下一页」：`rel=next`、分页栏里的「下一页」、`_2.html`、`?page=2` 这类 | `web_fetch`、`web_find` 自动接着抓、合成一篇（默认最多 10 页，`max_pages: 1` 关掉）；每页开头标「第 2 页」；不跟「下一篇」 |

**做法**（`docs/decisions/0008`）：

- 站点规则是 Rust 模块，每站一个，共用一套小工具：按网址和网页特征认站、选正文的根、先清掉站点特有的噪声、读元数据。规则在 Readability 之前跑；认不出、选不到就退回现在的通用做法。
- 只读 HTML（包括网页里的 JSON 数据块，比如 GitHub 新版 issue 页），不另调网站的 API：存好的网页、Peck 用 WebKit 渲染后的 HTML 交给 `web_extract` 也能用上规则。真实页证明 HTML 不够的，在决定里写例外。
- `Article` 加字段，不改已有的：`rule`（用了哪条规则）、`paper`、`images`、`next_page`（这一页里找到的下一页）、`pages`（合并后每页的网址）。
- 多页的位置：第几页 + 这一页里的标题路径和第几段，和单独抓那一页时一样，能逐页核对；写法「第 2 页〈方法〉第 3 段」，只有一页时不写页。
- 合并只走本机抓取：只跟同一网站、网址和第一页像的（同一目录、页号递增）；遇到抓过的网址、和上一页一样的正文就停；逐页查取消；中途哪一页没取到，交回前面几页并写明。`via: tavily` 不合并，交回下一页的网址。
- 知网、万方、维普、PubMed：程序拿不到真实页，移出 v0.4.0（站主 2026-11-28 定），记在下面「以后」。

| Phase | 做什么 | 验收 | 状态 |
| --- | --- | --- | --- |
| 1 底子与决定 | 规则框架和认站；`Article` 的新字段；合成样本的 `.expect.json` 加 `rule`、`paper`、`images`、`next_page`；真实页快照放 `local/pages/sites/`（只抓国际上程序拿得到的）；`decisions/0008` | 框架在合成页上跑通；没有规则的样本结果一字不变 | 完成（2026-11-28） |
| 2 文献页 | 通用的 `citation_*`、PRISM、Dublin Core 元数据 → `paper`；arXiv、PLOS、AEA、Nature 的规则；工具结果里的「文献」几行 | 每站合成页 + 几页真实页，元数据逐项对 | 完成（2026-11-28） |
| 3 百科、代码站、公众号 | 维基百科、GitHub（README、issue）、微信公众号的规则 | 同上 | |
| 4 图与多页 | `images`：懒加载的 `data-src`、`srcset` 取最大的、`figcaption`、去掉图标和 1×1 的统计图、记所在段；认下一页；`fetch_article`（抓 + 取正文 + 合并）；`web_fetch`、`web_find` 加 `max_pages`；工具结果末尾列「文中的图」（前 20 张，多的写还有几张） | 合成的多页文章和几篇真实的；模拟服务测跟页、停下、取消 | |
| 5 真实核对与收尾 | `docs/real-pages.txt` 的旧网址全部重跑：没用上规则的页，正文和 v0.3.0 逐字一样；用上规则的逐页看；文档（design、toolkit、CHANGELOG、README、工具清单）；`./scripts/check.sh` 两种配置都过；打 tag | 发 v0.4.0 | |

## 以后（没排版本）

- 知网、万方、维普的文献页，PubMed：程序拿不到真实页（`decisions/0008`）。能拿到真实页时再做：站主从浏览器存，或 App（Peck 的 WebKit）渲染后交给 `web_extract`。

## v0.3.0 抓取和搜索（已发，本机）

| Phase | 结果 |
| --- | --- |
| 1 抓取 | `Transport` + 默认的 `Ureq`（系统 TLS、解压、钉地址、截断、逐块查取消）；`fetch`（网址、内网、跳转、同站、上限、`HostPacer`）；feature `fake` 的 `Fake`；`decisions/0006` |
| 2 Tavily | 四个接口、本地报价（照官方计点规则）、按回复记点数、出错分类、可取消的重试；`decisions/0007` |
| 3 工具包 | `web_fetch`、`web_search`、`web_crawl`、`web_map` 四份新清单；`web_find` 可只给网址；`toolkit_with(Web)` |
| 4 真实核对 | 本机抓 72 个网址（58 个读到，没读到的是网站挡、要登录、要开 JavaScript、证书不对）；查出 2 处（react.dev 回 MDX 源文件被认成网页、https 降成 http 的说明不对）已修。真跑 Tavily 四个接口约 6 点（$0.05） |
| 5 收尾 | 文档；`./scripts/check.sh` 两种配置都过；打 tag |

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
| Peck | v0.5.0 | 没开始（Peck 还在开发，2026-11-28） |
| Pomelo | v0.6.0 | 没开始 |
| Pith | v0.7.0 | 没开始 |

## 用户验收清单

要站主亲自看、亲自做的，不挡开发：

- [ ] v0.2.0：把几份常看的网页存下来（浏览器的「存储为…」，格式选网页源码，不要网页归档），在本仓库目录的终端里跑 `cargo run -q --bin engine-web -- call web_extract '{"file":"<路径>","url":"<网址>"}'`，看正文对不对、有没有把导航和广告当正文；再把 `web_extract` 换成 `web_find`、加 `"query":"<文章里的一句话>"`，看找到的段落对不对。本机已核对过的 63 页的结果在 `local/out/`（每页一个 `.article.md`），也可以直接翻。
- [ ] v0.3.0：在本仓库目录跑 `cargo run -q --bin engine-web -- call web_fetch '{"url":"<常看的网页>"}'`，看正文对不对；再跑 `ENGINE_KEY_TAVILY="$(cat ~/Documents/Dataset/Pomelo/key/tavily)" cargo run -q --bin engine-web -- call web_search '{"query":"<一个问题>"}' --yes`，看结果和 `usage` 里的花费；爬站 `web_crawl` 不加 `--yes` 应当只报价。
- [ ] v0.4.0：在本仓库目录跑 `cargo run -q --bin engine-web -- call web_fetch '{"url":"https://arxiv.org/abs/1706.03762"}'`，看开头「文献：」几行（作者、出处、编号、PDF）和正文（摘要、Comments 那几项、提交记录）对不对；再换一篇你常看的期刊文章（Nature、PLOS、AEA 这类）看「文献」几行和正文开头有没有菜单、指标这类杂项。Phase 3、4 做完再加：一个维基百科条目、一个 GitHub issue、一篇公众号文章、一篇分页的文章，看正文全不全、「文中的图」的网址能不能打开、分页的是不是合成了一篇。
- [ ] v0.5.0：Peck 正式版里让模型搜一次、读一页，引用的网址对；内网地址被拒。
