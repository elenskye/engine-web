# HANDOFF

更新：2026-11-27 · v0.2.0 Phase 1–4 · 云端 Claude（分支 `cloud/v0.2`）

## 现在在哪

v0.2.0 的 Phase 1（骨架与样本）、2（取正文）、3（真实网页核对）、4（工具包与 find）都在 `cloud/v0.2` 上做完，**没在 Mac 上验**，`main` 没动。下一步是本机审核合并，在 Mac 上再过一遍真实网页，然后 Phase 5 收尾发版。

`cloud/v0.2` 上的 commit（还没并进 `main`）：

| commit | 说明 | 做了什么 |
| --- | --- | --- |
| `5c80368` | `v0.2.0: phase 1 types, samples and pages example` | `Error`、`Article` 等类型；合成样本和期望；`examples/pages`、`examples/make_samples`；版本 0.2.0 |
| `725670f` | `v0.2.0: phase 2 extraction, encoding and type detection` | 类型判断、编码、`article` / `full` 两种模式、分段、元数据；搬来的 Peck、Pith 测试 |
| `277987b` | `v0.2.0: chardetng, decisions and handoff` | 编码探测用 chardetng；决定 0002–0005；`scripts/snapshot.sh` |
| `8f23488` | `v0.2.0: phase 3 rules from github pages` | GitHub 页面查出的 4 条规则、2 份样本 |
| `da74605` | `v0.2.0: phase 4 toolkit, find and cli` | 依赖 engine-core `v0.2.0`；两份工具清单；`find`、`Loc`；`tools`；命令行；`docs/toolkit.md` |
| `9dd14f6` | `v0.2.0: phase 3 and 4 handoff` | 上一版 HANDOFF |
| `1de3bbc` | `v0.2.0: phase 3 rules from real pages` | 36 个真实网页查出的 7 条规则、6 份样本 |
| 最后一笔 | `v0.2.0: phase 3 real pages and handoff` | `docs/real-pages.txt`；设计稿、README、ROADMAP；本文件 |

站主这几轮定的：分支 `cloud/v0.2`；HTML 依赖跟 Peck 同一版（`docs/decisions/0002`）；`full` 模式搬 Pith 的走树（0003）；段号全文连续、小标题也算一段、路径不含文章标题（0004）；没声明编码用 chardetng（0005）；Phase 3 由云端自己抓公开网页（站主推荐 GitHub、X、知网和同类网站、ESS 这类大数据网页），站主把环境的网络放开成了 Full。

## Phase 3：真实网页核对

**网址**：`docs/real-pages.txt`（只存网址，快照在云端的 `local/pages/`，不进 Git，会话结束就没了；本机照文件开头的用法重抓）。抓了 39 个、存下 36 页：

| 类 | 页 | 结果 |
| --- | --- | --- |
| GitHub（只读得到本会话挂着的 engine-web；别人的仓库 GitHub 自己的权限规则拦了） | 仓库首页、README、`design.md`、决定 0005、`docs/` 目录、提交记录、`html.rs` 代码页、raw README | 文档页正文、表格、代码块都对；代码页判「可能要开 JavaScript」（GitHub 的代码是脚本渲染的，对）；目录页、提交记录页查出问题 1–4 |
| X | @rustlang 主页、jack 的第一条推文 | 主页判「可能要开 JavaScript」，对；推文页只有登录墙，推文不在静态 HTML 里（不绕过登录） |
| 知网和同类网站 | 知网海外站首页、跳转页、文献详情页；万方首页、期刊页、中小学图书馆；维普首页 | 文献详情页跳到滑块验证码页（海外访问；不绕过），`kns.cnki.net`、`scjg.cnki.net` 证书和主机名对不上、抓不到；跳转页查出问题 5；海外站首页查出问题 6、7；维普查出问题 8；万方都是门户和登录页，抽出来是零碎入口，正常 |
| 大数据、统计 | ESS 首页、第 11 轮数据发布新闻、数据门户列表、`ess.sikt.no` 门户；统计局发布列表、一篇公报；欧盟统计局人口统计 | ESS 新闻、欧盟统计局、统计局公报的正文和表格都对；`ess.sikt.no` 查出问题 5；统计局列表页查出问题 6；世界银行回 403（反爬），没抓到 |
| 百科、论文 | 维基英文、中文；arXiv 摘要页 | 都对；arXiv 的「Title:」「Abstract:」前缀、维基中文开头的信息框表格归 v0.5 的站点规则 |
| 新闻、政府文件 | BBC、新华网、人民网、中国政府网条例 | 都对（人民网、新华网现在是 UTF-8）；BBC 查出问题 9 |
| 文档、博客 | Rust 文档、Rust 博客、MDN、Python 文档、阮一峰的博客 | 都对；Rust 文档查出问题 10、11，MDN、Python 文档查出问题 11 |

**查出的问题和改法**（规则在 `src/html.rs`；每个问题一份合成样本，仿结构、字是自己写的；旧规则下这些样本都不过）：

| # | 问题 | 改法 | 样本 |
| --- | --- | --- | --- |
| 1 | 退回整个 body 时，站点页眉、标签栏、页脚（`<header>`、`role=banner / navigation / contentinfo`）进了正文 | 抽正文前去掉站点地标 `role`；退回 body 时再去掉正文外的 `<header>` | `repo-tree`、`repo-empty` |
| 2 | 「Skip to content」跳转链接进了正文 | 去掉 | `repo-empty` |
| 3 | 空列表项 `-`、空小标题 `##` 留在正文里，还各算一段 | 去掉 | `repo-tree`、`repo-empty` |
| 4 | 读屏专用文字和可见文字重复，类名带哈希（`visuallyHidden__TIbnmb`） | 类名去掉 `-`、`_` 后含 `visuallyhidden`、`screenreader` 或以 `sronly` 开头的去掉 | `repo-tree` |
| 5 | 只有一两个 `type="module"` 脚本的单页应用、脚本跳转页，正文是空的却没提示要开 JavaScript（Peck 的规则要 5 个脚本） | 正文不到 20 字而有脚本也提示 | `spa-module`、`js-redirect` |
| 6 | Readability 选中了页脚（统计局列表页：版权、备案号，超过 200 字的退回门槛） | 抽出来的不到 1000 字、带「© 2026」「版权所有」「ICP 备」这类字样，当作没抽到，退回整个 body | `release-list` |
| 7 | `javascript:` 链接留在正文里 | 只留文字 | `portal-icons` |
| 8 | 图标字体的空 `<i>` 转成一对空的 `*`（「产品服务**」） | 抽正文前去掉空的格式元素 | `portal-icons`、`book-chapter` |
| 9 | dom_query 把正文里的 `"` 转义成 `\"` | 还原；链接标题里的留着 | `quoted-news` |
| 10 | mdBook 的标题取成了书名（唯一的 h1 是菜单栏里的书名，`<title>` 是「章节 - 书名」） | h1 在正文外时取 `<title>` 里另一段；Readability 取了站名那段时改用第一段 | `book-chapter` |
| 11 | 小标题是指向自己锚点的链接（`## [Try it](#try_it)`），Python 文档标题后有 `[¶](#…)` | 只留文字 | `book-chapter` |

**改前改后对比**（同一批快照，`9dd14f6` 的代码对现在的代码，比标题、正文字数、段数、`needs_js`）：36 页里 13 页有变化，都是预期的——知网跳转页、`ess.sikt.no` 改标「可能要开 JavaScript」；统计局列表页、知网海外站首页、万方登录页不再只抽到页脚（字数变多，`needs_js` 随之变 false）；Rust 文档标题从书名改成章节名；其余 7 页是转义和锚点清理带来的字数小减。对比时还查出一处我这一轮自己引入的退步（链接标题里的 `\.` 没还原），已改，样本 `quoted-news` 盖住了。

**web_find 在真实页上**：统计局公报（整句、`full` 模式表格里按词）、政府网条例、欧盟统计局（`under`）、维基中文（繁体问、简体命中）、ESS 新闻（模型把「–」打成「-」）、BBC（直引号）、Rust 文档和 MDN（`under`）、人民网、arXiv（按词）都命中在对的段落。

## 公共接口（请本机审）

Phase 1–2：`extract`、`sniff`、`decode`、`ExtractOptions`、`Mode`、`Kind`、`Article`、`Block`、`BlockKind`、`Link`、`Error`、`Binary`。Phase 4 新加：

```rust
pub fn find(article: &Article, query: &engine_core::find::Query, under: Option<&[String]>) -> Result<Found<Loc>, Error>;
pub struct Loc { pub heading_path: Vec<String>, pub paragraph: u32, pub kind: BlockKind }   // label()；Serialize
pub enum Error { …, NoHeading }                                                              // kind() = "no_heading"
pub mod tools { pub fn toolkit() -> Toolkit<()>; pub fn manifest() -> Vec<ToolSpec>; pub fn call(name, args, ctx) -> Result<Output, ToolError>; }
pub use engine_core;                                                                         // App 用同一份 engine-core
```

- 两个工具的参数、结果、出错、例子：`tools/*.json`；接线说明：`docs/toolkit.md`；命令行 `src/bin/engine-web.rs`。
- `Toolkit` 的状态是 `()`；v0.3 的抓取设置（User-Agent、代理……）会放进状态，到时 `toolkit()` 要改签名。
- `not_web` 的说明和建议在代码里按类型写（「这是 PDF，不是网页。」「请把这份文件交给 PDF 工具读。」），别的错误只交种类，说明照清单补。

和 Peck、Pith 旧行为的差别（有意的）：设计稿 §3、§4 和 `docs/decisions/0003`、`0005` 列全了。这一轮多的一处要注意：**`needs_js` 比 Peck 多一条**（正文几乎是空的、有脚本也提示），Peck 的那条搬来的测试已按这个改了断言并写明原因。

## 测试

在云端 Linux 上跑的（不是 Mac）：

```bash
./scripts/check.sh
```

通过：`cargo fmt --check`；`cargo test`：单元 26、`tests/ported.rs` 6、`tests/samples.rs` 3（42 份样本，两种模式各跑一遍）、`tests/toolkit.rs` 6（清单自检、两个工具的 18 个例子、分段接起来等于全文、覆盖和导出、命令行）；`cargo clippy --all-targets -- -D warnings`。

手动跑过：`cargo run --release --example pages`（36 页，逐页看过总表和问题页）；`engine-web call web_find` 在 11 个真实页上找。

没跑：Mac 上的一切。

## 已知问题

- 标题带站名后缀、又判断不出哪段是站名的，照原样交回（「基金会管理条例_其他_中国政府网」「Announcing Rust 1.100.0 | Rust Blog」）。
- `full` 模式会把页面外框里的 `<h*>` 也当成小标题（统计局的「Aa 字体：小 中 大」），这些段的标题路径不好看。
- 门户、列表页（万方、维普、统计局列表）退回整个 body 后，用 `<div>` 写的导航还在正文里；列表页本来就是一串链接，可以接受。
- 靠 CSS 隐藏的重复内容去不掉（GitHub 文件表的文件名出现两次，统计局列表每条标题出现三次在 `full` 模式里）。
- `article` 模式的表格是 dom_query 转的：跨行跨列的表头会拍成一行、和下面的列对不齐（字都在）；维基中文的信息框也是这样。`full` 模式没这个问题。
- Readability 清掉了代码块的语言标记；代码块末尾多一个空行。
- `full` 模式照 Pith 去掉每行首尾空白（正文里代码缩进没了，分段里留着）；全角空格在行中折成一个空格。
- 新闻的作者行（「来源：… 记者 …」）会留在正文开头；维基的引用标记 `[\[3\]](#cite_note-3)` 留在正文里。
- 登录墙（X 的推文）、验证码（知网文献页）、反爬 403（世界银行）拿不到正文，引擎不绕过；X、知网、维基、arXiv、GitHub 的专门取法在 v0.5。

## 等站主

1. **本机 Claude 审核、合并**：

   ```bash
   git fetch origin cloud/v0.2
   ```

   看 diff（重点：公共接口、`tools/*.json` 的说明和例子、Phase 3 的 11 条规则），在 Mac 上跑 `./scripts/check.sh` 和 `cargo run --bin engine-web -- tools`，过了 merge 进 `main`。
2. **Mac 上再过一遍真实网页**（国内网络下知网、万方的页面可能不一样）：

   ```bash
   cp docs/real-pages.txt local/pages/urls.txt
   ```

   ```bash
   ./scripts/snapshot.sh
   ```

   ```bash
   cargo run --release --example pages
   ```

3. 环境的网络现在是 Full（站主为这一轮放开的）；不需要了可以改回 Limited。

## 下一轮

- 本机：审核合并 `cloud/v0.2`；Mac 上真实网页再过一遍；Phase 5 收尾（README、`CHANGELOG.md`、打 tag `v0.2.0`）。
- 云端：本机审核提出的问题回到 `cloud/v0.2` 改；v0.2 发版后开 `cloud/v0.3`（抓取和 Tavily）。
