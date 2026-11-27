# HANDOFF

更新：2026-11-27 · v0.2.0 Phase 1–5（云端部分）· 云端 Claude（分支 `cloud/v0.2`）

## 现在在哪

v0.2.0 在 `cloud/v0.2` 上做完：取正文、`find`、工具包（`web_extract`、`web_find`）、命令行、36 个真实网页的核对、CHANGELOG。**没在 Mac 上验**，`main` 没动、没打 tag（`CLAUDE.md`：云端不碰 `main`、不打 tag）。剩下的是本机验收和发版，步骤见下面「本机验收」。

`cloud/v0.2` 上的 commit（`main` 之后，按时间）：

| commit | 说明 | 做了什么 |
| --- | --- | --- |
| `5c80368` | `v0.2.0: phase 1 types, samples and pages example` | `Error`、`Article` 等类型；合成样本和期望；`examples/pages`、`examples/make_samples`；版本 0.2.0 |
| `725670f` | `v0.2.0: phase 2 extraction, encoding and type detection` | 类型判断、编码、两种模式、分段、元数据；搬来的 Peck、Pith 测试 |
| `277987b` | `v0.2.0: chardetng, decisions and handoff` | 编码探测用 chardetng；决定 0002–0005；`scripts/snapshot.sh` |
| `8f23488` | `v0.2.0: phase 3 rules from github pages` | GitHub 页面查出的 4 条规则 |
| `da74605` | `v0.2.0: phase 4 toolkit, find and cli` | 依赖 engine-core `v0.2.0`；两份工具清单；`find`；`tools`；命令行；`docs/toolkit.md` |
| `9dd14f6` | `v0.2.0: phase 3 and 4 handoff` | 交接 |
| `1de3bbc` | `v0.2.0: phase 3 rules from real pages` | 真实网页查出的 7 条规则 |
| `6b40186` | `v0.2.0: phase 3 real pages and handoff` | 修一处退步；`docs/real-pages.txt`；文档 |
| 最后一笔 | `v0.2.0: phase 5 changelog and acceptance checklist` | `CHANGELOG.md` 的 v0.2.0；README、ROADMAP；本文件；`web_extract` 两处小修（`needs_js` 提示的措辞；`</web-page>` 不分大小写地挡住） |

站主这几轮定的：分支 `cloud/v0.2`；HTML 依赖跟 Peck 同一版（`docs/decisions/0002`）；`full` 模式搬 Pith 的走树（0003）；段号全文连续、小标题也算一段、路径不含文章标题（0004）；没声明编码用 chardetng（0005）；Phase 3 由云端自己抓公开网页；Phase 5 云端做收尾、本机验收。

## 本机验收

在本仓库主目录、`main` 上做。

1. **取下来、看改动**：

   ```bash
   git fetch origin cloud/v0.2
   ```

   ```bash
   git diff --stat main...origin/cloud/v0.2
   ```

   重点审（会导致不同做法的地方）：
   - 公共接口：`CHANGELOG.md`「v0.2.0 · 公共接口」，代码在 `src/lib.rs`、`src/article.rs`、`src/find.rs`、`src/tools.rs`、`src/error.rs`。
   - 两份工具清单 `tools/web_extract.json`、`tools/web_find.json`：给模型看的说明、默认值、上限、出错的说明和建议。
   - Phase 3 的规则（`src/html.rs`）：像页脚就退回整个 body（`looks_like_footer`）、正文几乎是空的有脚本也提示 `needs_js`、标题的两条修正（`title_of`）、读屏专用元素按类名去掉（`screen_reader_only`）、转义还原（`unescape`）。每条都有合成样本，见下面「Phase 3」。
   - 和 Peck、Pith 旧行为的差别：`CHANGELOG.md`「和三家旧实现的差别」。

2. **合并**（`main` 在 `b0511dd` 之后没有新提交，会是快进；保留原提交）：

   ```bash
   git merge origin/cloud/v0.2
   ```

3. **Mac 上跑检查**：

   ```bash
   ./scripts/check.sh
   ```

   云端（Linux）的结果：`cargo fmt --check` 过；`cargo test`：单元 26、`tests/ported.rs` 6、`tests/samples.rs` 3（42 份样本，两种模式各跑一遍）、`tests/toolkit.rs` 6（清单自检、18 个例子、分段、覆盖和导出、命令行）；`cargo clippy --all-targets -- -D warnings` 过。另外 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` 也过。

4. **命令行冒烟**（预期是云端跑出来的）：

   ```bash
   cargo run --bin engine-web -- tools
   ```

   预期：列出 `web_extract`、`web_find`，费用 `local`、确认 `never`。

   ```bash
   cargo run --bin engine-web -- call web_extract '{"file":"tests/fixtures/html/gbk-undeclared.html"}' --json-only
   ```

   预期：`encoding` 是 `GBK`，`title` 是「示例县图书馆冬季开放时间调整」，`chars` 271，`blocks` 4。

   ```bash
   cargo run --bin engine-web -- call web_find '{"file":"tests/fixtures/html/big5-meta.html","query":"长者免费"}'
   ```

   预期：「找到 1 处」，「第 3 段：……六十五歲以上**長者免費**入場。」（简体问、繁体答）。

   ```bash
   cargo run --bin engine-web -- call web_extract '{"file":"tests/fixtures/html/paper.pdf"}'
   ```

   预期：退出码 1，`{"error":{"kind":"not_web","message":"这是 PDF，不是网页。","hint":"请把这份文件交给 PDF 工具读。",…}}`。

5. **Mac 上过一遍真实网页**（国内网络下知网、万方可能和云端从海外看到的不一样）：

   ```bash
   mkdir -p local/pages && cp docs/real-pages.txt local/pages/urls.txt
   ```

   ```bash
   ./scripts/snapshot.sh
   ```

   ```bash
   cargo run --release --example pages
   ```

   看 `local/out/summary.md`，和下面「Phase 3 · 云端的结果」对照。新闻页、GitHub 页内容会变，字数差一些正常；标题、`needs_js` 应当一样，差很多的那页打开 `local/out/<快照>.article.md` 看。有问题的写进本文件，回到 `cloud/v0.2` 改（或本机直接改，按 `CLAUDE.md` 的分工）。

6. **站主验收**（`ROADMAP.md`「用户验收清单」v0.2.0）：站主拿几份常看的网页试 `web_extract` 和 `web_find`。

7. **发版**（`CLAUDE.md`「三」：check 过 → CHANGELOG → commit → tag）：
   - `CHANGELOG.md` 把「## v0.2.0（云端做完，本机验收后打 tag）」改成「## v0.2.0」；README 开头、`ROADMAP.md`「现在」和版本表、本文件改成已发版的状态。
   - commit：`v0.2.0: release`。
   - 打 tag：

     ```bash
     git tag v0.2.0
     ```

   - push（`main` 和 tag）由站主做。`cloud/v0.2` 删不删由站主定。

## Phase 3：真实网页核对

网址在 `docs/real-pages.txt`（只存网址；快照放 `local/pages/`，不进 Git）。抓了 39 个、存下 36 页；没存下的：`kns.cnki.net`、`scjg.cnki.net`（海外访问证书和主机名对不上）、世界银行（反爬 403）。另外 X 的推文页只有登录墙、知网文献页跳到滑块验证码，引擎不绕过。

查出 11 个问题，都改了规则、各有合成样本（旧规则下这些样本都不过）：

| # | 问题（在哪页看到的） | 改法 | 样本 |
| --- | --- | --- | --- |
| 1 | 退回整个 body 时站点页眉、标签栏、页脚进了正文（GitHub 目录页） | 抽正文前去掉 `role` 是 banner、navigation、contentinfo 等的元素；退回 body 时再去掉正文外的 `<header>` | `repo-tree`、`repo-empty` |
| 2 | 「Skip to content」进了正文（GitHub） | 去掉页内跳转链接 | `repo-empty` |
| 3 | 空列表项 `-`、空小标题 `##` 留着还各算一段（GitHub） | 去掉 | `repo-tree`、`repo-empty` |
| 4 | 读屏专用文字重复，类名带哈希（GitHub） | 类名含 `visuallyhidden`、`screenreader` 或以 `sronly` 开头的去掉 | `repo-tree` |
| 5 | 空的单页应用、脚本跳转页没提示要开 JavaScript（ESS 数据门户、知网跳转页） | 正文不到 20 字而有脚本也提示 | `spa-module`、`js-redirect` |
| 6 | Readability 选中了页脚（统计局列表页） | 像页脚（不到 1000 字、带版权或备案字样）就退回整个 body | `release-list` |
| 7 | `javascript:` 链接留在正文（知网海外站） | 只留文字 | `portal-icons` |
| 8 | 空图标 `<i>` 变成一对空的 `*`（维普） | 去掉空的格式元素 | `portal-icons`、`book-chapter` |
| 9 | 正文里的 `"` 被转义成 `\"`（BBC） | 还原；链接标题里的留着 | `quoted-news` |
| 10 | mdBook 标题取成了书名（Rust 文档） | h1 在正文外时取 `<title>` 里另一段；Readability 取了站名那段时改用第一段 | `book-chapter` |
| 11 | 小标题是锚点链接、带 `¶`（MDN、Rust 文档、Python 文档） | 只留文字 | `book-chapter` |

改前改后用同一批快照对比过：13 页有变化，都是上面预期的；对比时查出的一处退步（链接标题里的 `\.`）已修。`web_find` 在 11 个真实页上试过整句、按词、繁简、模型把「–」打成「-」、直引号、`under`，都命中在对的段落。

**云端的结果**（2026-11-27，从海外抓；字数不算空白）：

| 快照 | 标题 | article 字数 | 段数 | needs_js |
| --- | --- | --- | --- | --- |
| 001-github.com.html | engine-web | 803 | 14 | false |
| 002-github.com.html | engine-web/README.md at main · elenskye/… | 803 | 14 | false |
| 003-github.com.html | engine-web/docs/design.md at cloud/v0.2 … | 10736 | 93 | false |
| 004-github.com.html | engine-web/docs/decisions/0005-encoding-… | 975 | 16 | false |
| 005-github.com.html | engine-web/docs at cloud/v0.2 · elenskye… | 1386 | 23 | false |
| 006-github.com.html | Commits | 1150 | 15 | false |
| 007-github.com.html | engine-web/src/html.rs at cloud/v0.2 · e… | 271 | 6 | true |
| 008-raw.githubusercontent.com.txt | engine-web | 1605 | 20 | false |
| 009-x.com.html | Rust Language (@rustlang) on X | 166 | 4 | true |
| 010-x.com.html | jack (@jack) on X | 892 | 11 | false |
| 011-oversea.cnki.net.html | Loading | 0 | 0 | true |
| 012-oversea.cnki.net.html | CNKI Overseas | 5314 | 84 | false |
| 015-www.wanfangdata.com.cn.html | 万方数据知识服务平台-科研学习全流程支持服务平台 | 630 | 16 | false |
| 016-sns.wanfangdata.com.cn.html | 万方数据知识服务平台 | 207 | 6 | true |
| 017-edu.wanfangdata.com.cn.html | 会员中心--登录 -- 万方数据中小学数字图书馆 | 972 | 6 | false |
| 018-www.cqvip.com.html | 维普网 | 603 | 70 | false |
| 019-www.europeansocialsurvey.org.html | Home \| European Social Survey | 2171 | 29 | false |
| 020-www.europeansocialsurvey.org.html | Final release of Round 11 data | 2486 | 16 | false |
| 021-www.europeansocialsurvey.org.html | ESS Data Portal \| European Social Surve… | 6784 | 81 | false |
| 022-ess.sikt.no.html | ESS Data Portal | 0 | 0 | true |
| 023-www.stats.gov.cn.html | 数据发布 - 国家统计局 | 12104 | 211 | false |
| 024-www.stats.gov.cn.html | 2026年1—10月份全国规模以上工业企业利润增长16.0% - 国家统计局 | 5154 | 536 | false |
| 026-ec.europa.eu.html | Population and population change statist… | 10182 | 44 | false |
| 027-en.wikipedia.org.html | European Social Survey | 17207 | 123 | false |
| 028-zh.wikipedia.org.html | 中国知识基础设施工程 | 14794 | 83 | false |
| 029-arxiv.org.html | Attention Is All You Need | 2108 | 10 | false |
| 030-www.bbc.com.html | Zelensky tells BBC US too focused on Mid… | 4720 | 46 | false |
| 031-www.news.cn.html | 创意微视频丨鹏程万里-新华网 | 282 | 14 | true |
| 032-finance.people.com.cn.html | “外企用脚投票中国”的市场逻辑（经济新方位·八大重点任务看落实） --经济·科技… | 1376 | 14 | false |
| 033-www.gov.cn.html | 基金会管理条例_其他_中国政府网 | 13105 | 299 | false |
| 034-doc.rust-lang.org.html | What is Ownership? | 21677 | 120 | false |
| 035-blog.rust-lang.org.html | Announcing Rust 1.100.0 \| Rust Blog | 17820 | 116 | false |
| 036-developer.mozilla.org.html | \<table\> HTML table element | 40647 | 185 | false |
| 037-docs.python.org.html | json — JSON encoder and decoder | 29126 | 218 | false |
| 038-www.ruanyifeng.com.html | 阮一峰的网络日志 | 2301 | 29 | false |
| 039-oversea.cnki.net.html | 安全验证 | 0 | 0 | true |

## 已知问题

不挡发版，记在这里，后面的版本处理：

- 标题带站名后缀、又判断不出哪段是站名的，照原样交回（「基金会管理条例_其他_中国政府网」「… | Rust Blog」）。
- `full` 模式会把页面外框里的 `<h*>` 也当成小标题（统计局的「Aa 字体：小 中 大」），这些段的标题路径不好看。
- 门户、列表页退回整个 body 后，用 `<div>` 写的导航还在正文里。
- 靠 CSS 隐藏的重复内容去不掉（GitHub 文件表的文件名出现两次）。
- `article` 模式的表格是 dom_query 转的：跨行跨列的表头会拍成一行、和下面的列对不齐（字都在）；维基中文的信息框也这样。`full` 模式没这个问题。
- Readability 清掉了代码块的语言标记；代码块末尾多一个空行。
- `full` 模式照 Pith 去掉每行首尾空白（正文里代码缩进没了，分段里留着）；全角空格在行中折成一个空格。
- 新闻的作者行留在正文开头；维基的引用标记 `[\[3\]](#cite_note-3)` 留在正文里。
- 登录墙（X 的推文）、验证码（知网文献页）、反爬 403（世界银行）拿不到正文，引擎不绕过。X、知网、维基、arXiv、GitHub 的专门取法在 v0.5。
- 和 Peck、Pith 旧实现的逐字对照在各 App 接入时做（design §7）。

## 等站主

1. 让本机 Claude 照「本机验收」做，过了发版；push `main` 和 tag `v0.2.0`。
2. Default 环境的网络这一轮放开成了 Full，不需要了可以改回 Limited。

## 下一轮

- 本机：验收、合并、发版 v0.2.0。
- 云端：本机验收提出的问题回到 `cloud/v0.2` 改；v0.2.0 发版后开 `cloud/v0.3`（抓取 `web_fetch`、Tavily 的 `web_search`、`web_crawl`、`web_map`，报价和 `Usage`，`FakeTransport`）。
