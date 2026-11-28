# HANDOFF

更新：2026-11-28 · **v0.4.0 Phase 1–4 完成**（没打 tag，版本号还是 0.3.0）· 本机 Claude Code

> 准则更新（2026-11-28）：「五」加了「花钱的规矩」（一次超过 $0.5 或算不出、Tavily 24 小时合计超过 $5 才问，引擎算、引擎记共用账本，总仓库 `decisions/0011`）和「清理」（`./scripts/clean.sh`，开工、每轮、发版三档，`0013`）；「三」加了「引擎带的大文件」（`0012`，本引擎没有）。
>
> **2026-11-28 下午另做了一件**（所有引擎停着时，在总仓库的会话里，站主让做）：钉 engine-core v0.4.1（git tag），Tavily 照花钱的规矩：`Web` 多了 `ledger`、`app`、`book()`，`Transport` 多了 `simulated()`（`Fake` 交 `true`），真花了钱记账；`web_crawl`、`web_map` 的确认从 `always` 改成 `never`（规矩之外不另问）；命令行不用问的直接执行。`check.sh` 开头把 `ENGINE_LEDGER_DIR` 指到临时目录；加了 `scripts/clean.sh` 和开工钩子。`./scripts/check.sh` 通过。接口变化记在 `CHANGELOG.md`「v0.4.0（开发中）」。

## 现在在哪

v0.3.0 已发（取正文、抓取、Tavily，六个工具）。v0.4.0 只剩 Phase 5（收尾、文档、发版）。已经做完的有：
- 站点规则：arXiv、PLOS、AEA、Nature、维基百科、GitHub、公众号
- 文献信息
- 文中的图
- 多页合并
- 认出挡程序的验证页

计划和状态在 `ROADMAP.md`「v0.4.0」，做法在 `docs/decisions/0008`。

## 这一轮（本机：v0.4.0 Phase 3–4）

站主 2026-11-28 定了两件事：
- 同意「认出挡程序的验证页」。原计划放 Phase 5，这一轮做了。
- GitHub 长 issue 中间没放进网页的回复：写明缺几条、给接口网址，引擎不另调接口（提问定）。

**框架**（`src/html/sites/mod.rs`）
- `Site` 加了两项：`build`（正文在网页的 JSON 数据块里时从那里拼）和 `meta`（站点自己写的标题、作者、日期、站名，两种模式都用）。
- 规则没写的项用 `..BLANK`。

**三条规则**
- `wikipedia.rs`：
  - 信息框读成两列的表。
  - 引用角标留成「[1]」，参考文献每条前面写同样的角标，两边对得上（注释组的「[a]」也一样）。
  - 红链只留字；表格里的行头 `<th>` 当普通格子；公式写成 `$TeX$`。
  - 去掉 edit 链接、导航框、侧栏、提示、维护模板、回链。
- `github.rs`：
  - README 和 Markdown 文件：读渲染好的 `article.markdown-body`。
  - issue：从数据块拼状态、标签、关联 PR、提问和每条回复（作者、身份、时间）；中间缺的写明几条，附接口网址。
  - 代码文件：拼成一段代码。
  - 标题写成「仓库 / 文件」或「标题 · 仓库#号」。
- `weixin.rs`：放出藏起来的正文；读公众号名、作者；发布时间从脚本变量 `ct` 换成北京时间。

**验证页**（`src/html/bot.rs`）
- 认法：正文不到 1500 字，网页里又有某家验证页特有的写法或那几句话，就把是哪一家记进 `Article::bot_check`。
- 工具报新的出错种类 `bot_check`（对应 `Error::BotCheck`）；`via: auto` 见到它会改用 Tavily。

**文中的图**（`src/html/images.rs`）
- `Article::images`：每张图的网址、替代文字、图注、在第几段后面。
- `srcset` 取最大的一张，懒加载的取 `data-src`；图标、统计图、徽章去掉。
- 不改正文：段号是拿图前面的字去分段里找出来的。
- 工具结果第一段的末尾列出前 20 张。

**多页**（`src/html/next.rs`、`src/pages.rs`）
- `Article::next_page`：同一主机、页号加一的才算下一页。
- 新接口：`fetch_article`、`follow`、`Stopped`。
- `web_fetch`、`web_find` 加 `max_pages`（默认 10，可设 1–20）。
- 列表页（链接的字过半）不合并。
- `Block`、`Loc`、`Image` 加 `page`，只有合了几页时才有；位置写成「第 2 页〈小标题〉第 3 段」。

**工具清单**
- `tools/web_fetch.json`、`web_find.json`、`web_extract.json`：加 `max_pages`、`bot_check` 和结果字段（`images`、`next_page`、`pages`、`stopped`），各加了例子。

**合成样本 9 份**
- `wiki-river`、`github-issue`、`github-readme`、`github-code`、`weixin-article`、`bot-check`，`story-1`–`3`（一篇分三页的文章）。
- `tests/samples.rs`：能查 `bot_check`，也能分模式查 `images`。
- `tests/tools_net.rs`：新测试覆盖合并、到上限停、中途失败、`web_find` 跨页找、验证页改用 Tavily。

**文档**
- `docs/decisions/0008`：Phase 3 的规则表，验证页、图、多页的做法。
- `docs/design.md`、`docs/toolkit.md`、`ROADMAP.md`。
- `docs/real-pages-sites.txt`：加 25 个网址。
- `examples/pages`：另写 `*.images.txt`；打印下一页、验证页，规则页另打印元数据。

## 测试

**检查**：`./scripts/check.sh` 通过（Mac）。内容是 `cargo fmt --check`，以及两种配置的 `cargo test` 和 `cargo clippy --all-targets -D warnings`；两种配置一共 185 次通过、0 个失败。

**没变的核对**：改代码前先用 Phase 2 的代码跑了一遍存底。
- 没用上新规则的，两种模式的结果和改之前一字不差：旧真实页 63 页、v0.4 真实页 60 页、旧合成样本 42 份。
- 用上规则的（维基百科 7 页、GitHub 7 页），`full` 模式的正文也没变，只有 GitHub 的标题变了。

**真实页**：`local/pages/sites/` 新抓了 25 个网址（047–071），都抓到了。
- 逐页看过正文：
  - 维基百科：英、德、日、中文，手机版，公式多的一页。
  - GitHub：首页、文件页、4 个 issue。
  - 公众号：4 篇。
- 验证页全部认出：PubMed 4 页、BMC 1 页、AnandTech 2 页、知网 1 页、公众号 1 页；别的页一页没认错。

**现网**（本机抓，免费）：
- 多页合并：@IT 3 页、ComputerBase 4 页、Discourse 论坛 7 页，都合成了一篇。
- 维基百科、GitHub 的结果对。
- PubMed 报 `bot_check`。
- Hacker News 认成列表页，只写出下一页的网址，不合并。
- 真跑 Tavily：没跑（这一版不花钱）。

## 已知问题（不挡开发）

- **公众号会时不时回验证页**：同一篇文章，前一次抓到正文，后一次回「环境异常」。规则在抓到的 4 篇上核过；回验证页时报 `bot_check`，让模型改用 Tavily。我没去试换 User-Agent 绕过验证。
- GitHub 的 pull request 页不在这一版范围，照通用做法抽，只抽到一部分对话。`/issues/N` 实际是 PR 时也会跳到这种页。
- 维基百科的信息框：开头只有一格的行（「Location」这类小标题）挪到了表前，成了加粗的一段，因为 Markdown 的表头不能有空格子。只放图的排版表（GDP 列表页开头的地图）转出来是空格子的表。
- 图在第几段是按图前面的字找的。电脑版和手机版各放一张的（Quanta），第二张的段号会对到第一张那里。
- 合了几页时，用 `offset` 接着取会把几页都重抓一遍：`web_fetch` 不缓存，缓存归 App。
- 链接不多的结果页（欧洲社会调查数据门户的 `?page=2`）会被当成多页文章合并；Hacker News 这种链接多的才认成列表页。
- 小毛病：
  - 公众号里「** TOP 1 **」这类粗体，两边带空格，显示不成粗体。
  - @IT 的作者是「[村上純一（…），＠IT]」，带方括号（网页的 meta 就这么写）。
- v0.3 留下的老问题照旧：
  - Tavily 地图在有的站上很浅。
  - Nature 这类先跳到别的子域名设 cookie 的，默认停在第一跳。
  - Tavily 取页没有标题。
  - 门户页、表头、`full` 模式外框的 `<h*>`（见 `CHANGELOG.md` v0.2.0）。

## 等站主

1. **push**（在 `engine-web` 目录）：

   ```bash
   git push origin main
   ```

2. **说「进入 Phase 5」**再收尾：旧网址全部重跑、补文档（CHANGELOG、README）、版本号改成 0.4.0、打 tag。

## 真实页

快照只在本机、不进 Git。重抓（已有的编号跳过）：

```bash
mkdir -p local/pages/sites && cp docs/real-pages-sites.txt local/pages/sites/urls.txt && ./scripts/snapshot.sh local/pages/sites/urls.txt
```

```bash
cargo run --release --example pages -- local/pages/sites --out local/out/sites
```

## 真跑

密钥只在验收里用：从站主的密钥文件夹复制到会话临时目录里 600 权限的文件，用环境变量交给 `#[ignore]` 的测试。不打印、不写进文档、夹具、日志和错误信息。照 `CLAUDE.md`「五」「花钱的规矩」（v0.4.0 计划不花）。

```bash
install -m 600 ~/Documents/Dataset/Pomelo/key/tavily <会话临时目录>/tavily.key
```

```bash
ENGINE_WEB_TAVILY_KEY_FILE=<会话临时目录>/tavily.key cargo test --release --test real -- --ignored --nocapture
```

## 下一轮

Phase 5 要做的：
- 重抓 `docs/real-pages.txt` 的旧网址，和 v0.3.0 比：没用上规则的正文要一字不差。这一轮已经在旧快照上核过。
- 写 `CHANGELOG.md` 的 v0.4.0：`Article` 的新字段，`Block`、`Loc`、`Image` 的 `page`，`bot_check`，`fetch_article`、`follow`，`max_pages`。
- 更新 `README.md`。
- 版本号改成 0.4.0：`Cargo.toml`、`Cargo.lock`、`src/lib.rs` 的版本测试。
- 跑 `./scripts/check.sh`，打 tag `v0.4.0`。

比对的做法照旧：改代码前先跑一遍 `examples/pages` 存底（旧真实页、`local/pages/sites/`、合成样本各一份，输出放会话临时目录），改完再跑、逐个比。
