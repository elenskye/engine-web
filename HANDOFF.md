# HANDOFF

更新：2026-11-27 · **v0.2.0 已发**（tag `v0.2.0`）· 本机 Claude Code

## 现在在哪

v0.2.0 能用了：网页字节 → 标题、作者日期站名、Markdown 正文、分段、链接；`find` 在里面找字；工具包 `web_extract`、`web_find` 和命令行 `engine-web`。不联网。接口见 `CHANGELOG.md`，接线见 `docs/toolkit.md`。还没有 App 接。

## 这一轮（本机：审核、合并、核对、发版）

- **合并**：云端 `cloud/v0.2` 的 9 笔提交原样并进 `main`。合并前在本机的 `cloud/v0.2` 上加了一笔改名：样本 `readme.md` 和说明 `README.md` 在同一目录，Mac 上撞成一个文件、切不了分支，样本改成 `kestrel.md`（规则写进 `CLAUDE.md`「六」）。
- **结构**：`src/html.rs`（1000 多行、五件事混在一起）拆成 `src/html/`：`mod.rs`（`article` 模式的流程、`needs_js`、页脚判断、链接）、`clean.rs`（抽正文前的清理）、`markdown.rs`（整理 Markdown）、`meta.rs`（标题和元数据）、`full.rs`（全部文字和分段）。两处一样的空白折叠并成一个函数；行内代码查找不再每个字节分配一次。
- **真实网页核对**：云端那 36 个网址加本机补的 33 个（国内门户、政府和学校、GBK / GB18030 / Shift_JIS 老网页、Nature、arXiv HTML 版、斯坦福哲学百科、W3C 规范、React 文档、PDF、JSON、RSS），在国内网络下存下 63 页，两种模式全跑（63 页共约 1.2 秒）。查出并修了 9 处，每处配了合成样本（在云端原版上都不过）：

  | # | 问题（在哪页看到的） | 改法 | 样本 |
  | --- | --- | --- | --- |
  | 1 | 标题里写着「%PDF-1.7」的网页被当成 PDF 拒掉（自测） | `%PDF-` 要在开头，或前面只有空白、二进制字节 | `pdf-explainer` |
  | 2 | `.md` 文件开头是 `<div align="center">` 被当成网页（GitHub 上 Rust 的 README） | 扩展名或 Content-Type 说是 Markdown 的先认 Markdown | `html-readme` |
  | 3 | 短新闻末尾「本文版权所有」被当成页脚，退回整页，导航和热门推荐进了正文（自测，中文新闻常见） | 大半的字在版权、备案那几段才算页脚 | `brief-copyright` |
  | 4 | 代码高亮逐行一个 `<div>` 加 `<br>`，代码挤成一行、缩进丢了（React 文档） | 抽正文前把 `<pre>` 里的换行写成真的换行 | `code-lines` |
  | 5 | 每个引用角标带整条参考文献当链接标题，Nature 正文多出一成，维基列表页两成（Nature、维基、W3C） | 去掉链接标题 | `wiki-refs`、`quoted-news` |
  | 6 | 空的 `mailto:` 被拼成 `https://arxiv.org/html/mailto:`（arXiv HTML 版） | 只有协议的链接去掉 href，只留文字 | `wiki-refs` |
  | 7 | 大写的 `<SCRIPT>` 没算进脚本数，`needs_js` 漏报（自测） | 不分大小写、JSON-LD 不算，在原 HTML 里数 | `script-caps` |
  | 8 | 列表项里用 `<br>` 断行，下一行被拆成单独一段（国家自然科学基金委首页） | 照 CommonMark，不缩进的续行还算这一项 | `contact-list` |
  | 9 | 作者是一串数字 `103977`（人民网 meta 里填的编号） | 没有一个字的作者不算 | 单元测试 |

  另外：`scripts/snapshot.sh` 加了 `--compressed`（网易首页不管要不要都回 gzip，存下来的是压缩字节，引擎照实报「压缩包」）。
- **改过的文档**：`README.md`、`CHANGELOG.md`（v0.2.0 定稿）、`ROADMAP.md`（v0.2 的 Phase 表收成结果）、`docs/design.md` §3–§4、`docs/decisions/0004`（路径）、`docs/real-pages.txt`（加本机的网址）、`CLAUDE.md`「六」（文件名不能只差大小写）、本文件。

## 测试

- `./scripts/check.sh`：**通过**（Mac）。`cargo fmt --check`；`cargo test`：单元 32、`tests/ported.rs` 6、`tests/samples.rs` 3（49 份样本，两种模式各一遍）、`tests/toolkit.rs` 6（清单自检、18 个例子、分段续取、命令行）；`cargo clippy --all-targets -D warnings`。
- 真实网页：`cp docs/real-pages.txt local/pages/urls.txt && ./scripts/snapshot.sh && cargo run --release --example pages`，结果在 `local/out/`（不进 Git）。改前改后对比过：22 页有变化，都是上面预期的（多数是链接标题去掉后变短）。
- 命令行真调过：`tools`；`web_extract` 分段续取（人民网）；`web_find` 繁体问简体答（人民网「實際使用外資」）、`under` 限定小标题（BBC）、按词找、Shift_JIS 老网页（青空文库）、表格一行（维基 GDP 列表 `full` 模式）、GBK（科学网）；云端交接里的四条冒烟和预期一致。

## 已知问题（不挡发版）

- 门户、列表页退回整个 `<body>` 后，用 `<div>` 写的导航还在正文开头；靠 CSS 隐藏的重复内容去不掉（统计局列表每条链接出现三次）。v0.5 的站点规则再处理。
- `article` 模式的表格是 dom_query 转的：跨行跨列的表头拍成一行、和下面的列对不齐（字都在；维基中文的信息框也这样）。`full` 模式没这个问题。
- `full` 模式把页面外框里的 `<h*>` 也当小标题，标题路径不好看（统计局「字体：小 中 大」、维基把文章标题算进路径）。
- 标题带站名后缀、又判断不出哪段是站名的，照原样交回（「基金会管理条例_其他_中国政府网」）。
- Markdown 原文里的 HTML 标签原样留在正文和分段里，里面的 `<a href>` 不进 `links`。
- 新闻的作者行、维基的引用标记 `\[3\]` 留在正文里；Readability 清掉了代码块的语言标记。
- 登录墙（X）、验证码（知网文献页）、反爬 403（世界银行、百度百科、澎湃、Stack Overflow、卫报）拿不到正文，引擎不绕过。
- 和 Peck、Pith 旧实现的逐字对照在各 App 接入时做（design §7）。

## 等站主

1. **push**（先 engine-web）：在 `engine-web` 目录里

   ```bash
   git push origin main
   ```

   ```bash
   git push origin v0.2.0
   ```

2. **删云端分支**：`main` 推上去以后再删（云端的提交已经在 `main` 里）。本机分支已删。

   ```bash
   git push origin --delete cloud/v0.2
   ```

3. **一个暂存可以丢**：`git stash list` 里的「case collision artifact」是 Mac 上文件名撞车留下的假改动，没有内容，`git stash drop` 就行。
4. **v0.3 真跑 Tavily 前**：密钥文件夹 `~/Documents/Dataset/Pomelo/key/` 里还没有 `tavily`。放一份进去（一行密钥），本机 Claude 照下面「真跑」交给测试；每版上限 $1。
5. **v0.3.0 的计划**：本机写好 `ROADMAP.md` 的 Phase 表后看一眼，说一声云端就开工。

## 真跑（v0.3 起）

密钥只在验收里用：从站主的密钥文件夹读，写进会话临时目录里 600 权限的文件，用环境变量交给 `#[ignore]` 的测试（`cargo test -- --ignored`）。不打印、不写进文档、夹具、日志和错误信息。

## 下一轮

- 本机：写 v0.3.0 的 Phase 计划（`web_fetch`：跳转、超时、大小上限、内网检查、gzip / br 解压；Tavily 四个接口、报价、`Usage`、`FakeTransport`），站主看过再开工。
- 云端：站主批了 v0.3.0 计划后，从 GitHub 上的 `main` 开 `cloud/v0.3`。
