# HANDOFF

更新：2026-11-28 · **v0.3.0 已发**（tag `v0.3.0`，已推上 GitHub）· **v0.4.0 计划写好，等站主看** · 本机 Claude Code

## 现在在哪

v0.3.0 能用了：取正文、抓取（`fetch`）、Tavily（搜索、取页、爬站、站内地图），六个工具 `web_extract`、`web_find`、`web_fetch`、`web_search`、`web_crawl`、`web_map` 和命令行。接口见 `CHANGELOG.md`，接线见 `docs/toolkit.md`。还没有 App 接。

下一版换了：**v0.4.0 改做站点规则、文章里的图、多页合并**（原 v0.5），**接 Peck 挪到 v0.5.0**（原 v0.4；Peck 还在开发，同一个 App 一次只接一个引擎）。Phase 计划在 `ROADMAP.md`「v0.4.0」，站主看过再开工。

## 这一轮（本机：清残留、对调版本、拟 v0.4.0 计划）

- **上一轮的待办**：GitHub 上 `main`、tag `v0.2.0` / `v0.3.0` 已推，`cloud/v0.2` 已删（`git ls-remote` 核过）。本机残留清掉了：`git stash drop`（Mac 上 `README.md` / `readme.md` 撞车留下的假改动，内容核过是 `tests/fixtures/html/README.md` 被 `kestrel` 样本盖掉）；`git update-ref -d` 删了 `refs/original/refs/heads/main`、`refs/original/refs/tags/v0.2.0`（改作者时 `filter-branch` 留的旧指针）。剪贴板里已经不是 Tavily 密钥了（只比对、没打印），没动。
- **对调 v0.4.0 和 v0.5.0**：`ROADMAP.md`（版本表、各 App 接入进度、验收清单）、`README.md`、`docs/design.md`（§4、§7、§8、§9、§10）、`docs/toolkit.md`、`CHANGELOG.md`（v0.2.0「App 怎么接」里的版本号）；总仓库 `ROADMAP.md` 的 engine-web 两处。
- **新 v0.4.0 计划**（站主 2026-11-28 用提问定）：
  - 全部在本机做；不花钱（只用本机抓取）。
  - 站点：原定四个（知网文献页、arXiv 摘要页、维基百科、GitHub 的 README 和 issue）+ 微信公众号文章 + 万方、维普文献页 + PubMed、期刊文章页。
  - 多页文章：`web_fetch`、`web_find` 自动接着抓、合成一篇（默认最多 10 页）。
  - 图：列出网址、说明、在哪一段；正文照旧只留说明；不顺带下载。
  - 做法（我定，Phase 1 写进 `decisions/0008`）：每站一个 Rust 模块，规则在 Readability 之前跑，认不出就退回通用做法；只读 HTML 不调 API；`Article` 只加字段；多页的位置是「第 2 页〈方法〉第 3 段」，和单独抓那一页一样。
- **查了知网抓不到的原因**：本机网络下 `kns.cnki.net`、`scjg.cnki.net` 交的是腾讯云 CDN 的证书（`*.cdn.myqcloud.com`），网站名对不上（`curl` 一样失败，`www.cnki.net` 正常）。不是引擎的问题，也不跳过校验；知网的真实页要站主从浏览器存。

## 测试

- `./scripts/check.sh`：**通过**（Mac，2026-11-28；这一轮只改了文档）。`cargo fmt --check`、两种配置的 `cargo test` 和 `cargo clippy --all-targets -D warnings`，一共 139 个测试通过、0 个失败。
- 真跑：这一轮没跑。

## 已知问题（不挡开发）

- Tavily 的站内地图在有的站上很浅（Rust 博客首页只找到 3 个网址），是 Tavily 那边的；文档站正常。
- Nature 这类先跳到别的子域名设 cookie 再跳回来的，默认停在第一跳。用户自己给的网址，App 可以放开 `follow_other_sites`；模型调用时可以换 `via: tavily`。
- Tavily 取页没有标题；代码块被 Tavily 拍成一行。
- 取正文的老问题照旧（见 `CHANGELOG.md` v0.2.0）：门户页退回整个 body 后 `<div>` 写的导航还在、`article` 模式跨行跨列的表头对不齐、`full` 模式把外框的 `<h*>` 当小标题、带站名后缀的标题照原样。维基百科的信息框、引用角标、红链网址在 v0.4.0 的规则里处理。
- 和 Peck、Pith、Pomelo 旧实现的逐字对照在各 App 接入时做（design §7）。

## 等站主

1. **看 v0.4.0 的 Phase 计划**（`ROADMAP.md`「v0.4.0」），没问题就说「开工」。
2. **push**（在 `engine-web` 目录、总仓库目录各一次，这一轮只有文档）：

   ```bash
   git push origin main
   ```

3. **存几页程序抓不到的真实页**（Phase 1 要用，不急；能抓的我自己抓）：
   - 要的：知网文献页 3–5 页（期刊、学位论文、会议论文各一页最好）；万方、维普文献页各 2–3 页；微信公众号文章 3–5 篇（带图的）；你常看、要翻页的文章，有的话给网址就行。
   - 怎么存：浏览器打开那一页 →「文件」→「存储为…」→ 格式选「网页源码」（Safari）或「网页，仅 HTML」（Chrome），不要网页归档 → 存进 `engine-web/local/pages/sites/`，文件名随意。
   - 同一个文件夹里建 `urls.txt`，一行一页：`<文件名> <网址>`。

## 真跑

密钥只在验收里用：从站主的密钥文件夹复制到会话临时目录里 600 权限的文件，用环境变量交给 `#[ignore]` 的测试。不打印、不写进文档、夹具、日志和错误信息。每版上限 $1（v0.4.0 计划不花）。

```bash
install -m 600 ~/Documents/Dataset/Pomelo/key/tavily <会话临时目录>/tavily.key
```

```bash
ENGINE_WEB_TAVILY_KEY_FILE=<会话临时目录>/tavily.key cargo test --release --test real -- --ignored --nocapture
```

命令行：`ENGINE_KEY_TAVILY="$(cat <会话临时目录>/tavily.key)" cargo run --bin engine-web -- call web_search '{"query":"…"}' --yes`。

## 下一轮

- 站主说「开工」后做 v0.4.0 Phase 1：规则框架和认站、`Article` 的新字段、`.expect.json` 的新字段、真实页快照（`local/pages/sites/`）、`decisions/0008`。版本号到 Phase 5 发版时再改成 0.4.0。
- v0.5.0 接 Peck 等 Peck 手上的版本做完，在 Peck 的仓库里做（守 Peck 的 `CLAUDE.md`），开工前问站主程度。
