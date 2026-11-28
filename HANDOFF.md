# HANDOFF

更新：2026-11-28 · **v0.4.0 Phase 1、2 完成**（没打 tag，版本号还是 0.3.0）· 本机 Claude Code

## 现在在哪

v0.3.0 已发（取正文、抓取、Tavily，六个工具）。v0.4.0 在做：站点规则的框架、`Article` 的新字段、文献信息、arXiv / PLOS / AEA / Nature 四条规则做完了；维基百科、GitHub、公众号（Phase 3），图和多页（Phase 4），收尾发版（Phase 5）没做。计划和状态在 `ROADMAP.md`「v0.4.0」，做法在 `docs/decisions/0008`。

## 这一轮（本机：v0.4.0 Phase 1–2）

站主 2026-11-28 定：中国大陆有网络问题的网站不抓、不作样本，只抓国际上程序拿得到的；知网、万方、维普移出 v0.4.0（提问定）。PubMed 现在对一切程序回 cookie 验证页，照同样的理由不做（同一批文献走 PMC）。

- **Phase 1 底子**（commit `159088f`）：
  - `src/article.rs`：`Article` 加 `rule`、`paper`、`images`、`next_page`、`pages`；新类型 `Paper`、`Author`、`PaperId`、`Image`（可序列化）。`images`、`next_page`、`pages` 到 Phase 4 才填。
  - `src/html/sites/mod.rs`：`Site`（`key`、`matches`、`fix`、`root`、`drop`、`shape`）、认站、`fix`、`narrow`；选不到根或根里不到 20 字就退回通用做法。`src/html/mod.rs` 接进去（`from_root` 直接转 Markdown）。
  - `tests/samples.rs` 和 `tests/fixtures/html/README.md`：`.expect.json` 能写 `rule`、`paper`、`images`、`next_page`（故意写错一处，两种模式都报出来了）。
  - 工具 `data` 加 `rule`、`paper`、`images`、`next_page`、`pages`；`examples/pages` 打印规则和文献。
  - `docs/decisions/0008-site-rules.md`。
- **Phase 2 文献页**（这一轮的第二笔 commit）：
  - `src/html/paper.rs`：读 `citation_*`、PRISM、Dublin Core；有 `citation_title` 或 PRISM 刊名 + DOI 才算文献页，只有 Dublin Core 的不算。作者和机构、ORCID、卷期页、报告号、DOI（去前缀）、PDF（补成绝对网址）、arXiv / PMID / ISSN 这些编号、关键词、摘要（去标签）。有文献信息时标题用 `citation_title`（修好了 Frontiers 的标题只有「Frontiers」）。`article` 模式去掉参考文献后面的 Google Scholar、CAS 链接，`full` 模式不动。
  - 规则：`src/html/sites/arxiv.rs`、`plos.rs`、`aea.rs`、`nature.rs`。
  - `src/tools/extract.rs`：结果在标题下面列「文献：」几行（作者前 10 位、机构前 5 个、出处、编号、PDF、关键词；日期和上面一样就不重复）。
  - 合成样本 7 份：`paper-meta`、`paper-prism`、`dc-only`、`arxiv-abs`（逐字对）、`plos-article`、`aea-article`、`nature-article`。
  - 文档：`docs/design.md` §4、§8、§10，`docs/toolkit.md`「结果」，`tools/web_extract.json`、`tools/web_fetch.json` 的说明各加一句，`docs/real-pages-sites.txt`（v0.4 真实页的网址），`ROADMAP.md`。

## 测试

- `./scripts/check.sh`：**通过**（Mac）。`cargo fmt --check`、两种配置的 `cargo test` 和 `cargo clippy --all-targets -D warnings`；一共 161 个测试通过、0 个失败（v0.3.0 是 139 个）。
- **真实页**：`local/pages/sites/` 抓了 46 个网址里的 35 页，其中 14 页是文献页，文献信息逐页核过（作者、机构、刊名卷期页、DOI、编号、PDF）。用上规则的 9 页逐页看过正文开头和结尾。
- **没变的核对**：v0.3.0 代码先跑一遍存底，再和现在比（`examples/pages`）。合成样本：除了新加的 7 份，一字不变。65 页旧的真实页：变了的只有 3 页（旧的 arXiv 摘要页、Nature 用上规则，斯坦福哲学百科有 `citation_*`、正文没变），`full` 模式的正文都没变。
- 现网：`web_fetch` arXiv 摘要页（本机抓，免费）结果对。真跑 Tavily：没跑（这一版计划不花钱）。

## 已知问题（不挡开发）

- **挡程序的验证页被当成正文**：PubMed 的「要开 cookie」页、BMC 的「Client Challenge」、AnandTech 的「Checking your browser」现在交回的是那几句话，不报错。可以认出来、报一个新的出错种类，告诉模型换 `via: tavily`（v0.4.0 范围外，见「等站主」）。
- 作者名照网页写法（arXiv 是「Vaswani, Ashish」，PMC 是「John Jumper」），不统一。
- Nature 正文里的引用角标有的是 `[1](#ref-CR1)`，有的带整个网址，v0.3 起就这样。
- PLOS 的「**Citation: **」这类加粗后面带空格，Markdown 显示不成粗体，v0.3 起就这样。
- v0.3 留下的老问题照旧：Tavily 地图在有的站上很浅；Nature 这类先跳到别的子域名设 cookie 的默认停在第一跳；Tavily 取页没有标题；门户页、表头、`full` 模式外框的 `<h*>`（见 `CHANGELOG.md` v0.2.0）。

## 等站主

1. **push**（在 `engine-web` 目录）：

   ```bash
   git push origin main
   ```

2. **说「进入 Phase 3」**再接着做（维基百科、GitHub 的 README 和 issue、微信公众号）。公众号是中国大陆的网站：Phase 3 开头先试抓，有网络问题就照你的规矩不做，用提问跟你说。
3. **要不要认出挡程序的验证页**（推荐要，放进 Phase 5）：认出来就报出错，不把「Checking your browser」当正文交给模型。会多一个出错种类，属于范围的事，你定。

## 真实页

快照只在本机、不进 Git。重抓：

```bash
mkdir -p local/pages/sites && cp docs/real-pages-sites.txt local/pages/sites/urls.txt && ./scripts/snapshot.sh local/pages/sites/urls.txt
```

```bash
cargo run --release --example pages -- local/pages/sites --out local/out/sites
```

## 真跑

密钥只在验收里用：从站主的密钥文件夹复制到会话临时目录里 600 权限的文件，用环境变量交给 `#[ignore]` 的测试。不打印、不写进文档、夹具、日志和错误信息。每版上限 $1（v0.4.0 计划不花）。

```bash
install -m 600 ~/Documents/Dataset/Pomelo/key/tavily <会话临时目录>/tavily.key
```

```bash
ENGINE_WEB_TAVILY_KEY_FILE=<会话临时目录>/tavily.key cargo test --release --test real -- --ignored --nocapture
```

## 下一轮

- Phase 3：维基百科（信息框读成两列的表、引用角标、参考文献放最后、去掉编辑链接和导航框；真实页已有英、德、日三页和 Wikibooks 一页）、GitHub（README 页、issue 页，issue 的数据可能在网页的 JSON 数据块里）、公众号（先试抓）。规则加在 `src/html/sites/`，注册进 `SITES`；每条配合成页。改代码前先用现在的代码跑一遍 `examples/pages`（旧真实页、`local/pages/sites/`、合成样本各一份，输出放会话临时目录）存底，改完再跑、逐个比：没用上新规则的页一字不变。
- Phase 4 要的翻页长文：这一轮找的两家（Phoronix、AnandTech）都挡程序，Phase 4 开头再找。
