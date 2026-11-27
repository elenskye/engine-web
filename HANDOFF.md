# HANDOFF

更新：2026-11-27 · v0.2.0 Phase 1–4 · 云端 Claude（分支 `cloud/v0.2`）

## 现在在哪

v0.2.0 的 Phase 1（骨架与样本）、2（取正文）、4（工具包与 find）做完；Phase 3（真实网页核对）站主让云端自己抓公开网页来做，这一轮**只抓到了 GitHub 上 engine-web 自己的 6 页**，别的网站被云端环境的网络策略拦了，见下面「Phase 3」。都在 `cloud/v0.2` 上，**没在 Mac 上验**，`main` 没动。

`cloud/v0.2` 上的 commit（还没并进 `main`）：

| commit | 说明 | 做了什么 |
| --- | --- | --- |
| `5c80368` | `v0.2.0: phase 1 types, samples and pages example` | `Error`、`Article` 等类型；合成样本和期望；`examples/pages`、`examples/make_samples`；版本 0.2.0 |
| `725670f` | `v0.2.0: phase 2 extraction, encoding and type detection` | 类型判断、编码、`article` / `full` 两种模式、分段、元数据；搬来的 Peck、Pith 测试 |
| `277987b` | `v0.2.0: chardetng, decisions and handoff` | 编码探测用 chardetng；决定 0002–0005；`scripts/snapshot.sh` |
| `8f23488` | `v0.2.0: phase 3 rules from github pages` | GitHub 页面查出的 4 个问题的规则和 2 份合成样本 |
| `da74605` | `v0.2.0: phase 4 toolkit, find and cli` | 依赖 engine-core `v0.2.0`；`tools/web_extract.json`、`tools/web_find.json`；`find`、`Loc`；`tools` 模块；命令行 `engine-web`；`docs/toolkit.md` |
| 最后一笔 | `v0.2.0: phase 3 and 4 handoff` | 本文件 |

这一轮站主定的：Phase 3 由云端直接抓公开网页（站主推荐 GitHub、X、知网和同类网站、ESS 这类大数据网页）。之前定的见 `docs/decisions/0002`–`0005`。

## Phase 3：真实网页核对

**抓到的**（快照在云端的 `local/pages/`，不进 Git；会话结束就没了，本机要重抓）：GitHub 上 engine-web 自己的 6 页——仓库首页、`docs/design.md`、`docs/` 目录、提交记录、`src/html.rs` 代码页、raw 的 README。还有 2 页 GitHub 一直回 503，没抓到。

**被拦的**：云端环境（「Default」，网络是「Limited」）只放行包管理器；GitHub 只放行本会话挂着的仓库。我想为 engine-web 新建一个只放行这些主机的个人环境，服务拒绝了这个请求（没给原因）。要补 Phase 3，需要站主放行下面这些主机，见「等站主」第 1 条：

```text
x.com
www.cnki.net
kns.cnki.net
www.cnki.com.cn
www.wanfangdata.com.cn
www.cqvip.com
www.europeansocialsurvey.org
ess.sikt.no
www.stats.gov.cn
data.worldbank.org
ec.europa.eu
www.gov.cn
en.wikipedia.org
zh.wikipedia.org
arxiv.org
www.bbc.com
www.people.com.cn
politics.people.com.cn
doc.rust-lang.org
blog.rust-lang.org
```

（X 和知网检索页大概率是脚本渲染，正好测 `needs_js`；人民网有 GB2312 老页；ESS、世界银行、统计局测数据表格。）

**GitHub 6 页的结果**：

| 页 | 结果 |
| --- | --- |
| 仓库首页 | README 抽对了，标题、表格、代码块都在 |
| `docs/design.md` | 正文、表格、代码块都对；`web_find` 在上面整句、按词、表格格子里都能命中，标题路径对 |
| raw README | 认成 Markdown（Content-Type 是 text/plain，按扩展名），原样交回 |
| `src/html.rs` 代码页 | 正文只有 271 字，`needs_js` 为真——GitHub 的代码是脚本渲染的，判对了；结果里有 Raw 链接 |
| `docs/` 目录、提交记录 | Readability 抽不出、退回整个 body，**有问题**，见下 |

**查出的问题和改法**（每个都有合成样本：`repo-tree`、`repo-empty`，仿 GitHub 的结构、自己写的字）：

1. 退回整个 body 时，站点页眉（`<header role="banner">` 里的产品菜单）、`role="navigation"` 的标签栏、`role="contentinfo"` 的页脚没去掉。→ 抽正文前就去掉 `role` 是 banner、navigation、contentinfo、complementary、search、menubar、menu、toolbar 的元素；退回 body 时再去掉不在 `<article>`、`<main>` 里的 `<header>`。
2. 「Skip to content」这种页内跳转链接进了正文。→ 去掉 `href` 以 `#` 开头、文字是「Skip to …」「跳到…」的链接。
3. 内容去掉以后只剩记号的空列表项 `-` 和空小标题 `##` 留在正文里，还各算一段。→ `tidy_markdown` 里去掉。
4. 读屏专用文字和可见文字重复（「Folders and filesFolders and files」）。GitHub 的类名带哈希（`visuallyHidden__TIbnmb`、`prc-src-InternalVisuallyHidden-2YaI6`）。→ 类名去掉 `-`、`_` 后含 `visuallyhidden`、`screenreader` 或以 `sronly` 开头的元素，抽正文前去掉（`full` 模式不动，和 Pith 一致）。

改后目录页从「产品菜单 + 一串 `-`」变成「面包屑、仓库标签、文件表」。还剩的归 v0.5 的 GitHub 站点规则：仓库标签栏（Code、Issues…）还在；文件表因为响应式布局（靠 CSS 在不同屏宽下隐藏一列）每个文件名出现两次、表头「NameName」，不看 CSS 判断不了。

## 公共接口（请本机审）

Phase 1–2 的（`extract`、`sniff`、`decode`、`Article`、`Block`、`Error`、`Binary`……）见上一版 HANDOFF 的列表，没变。Phase 4 新加的：

```rust
pub fn find(article: &Article, query: &engine_core::find::Query, under: Option<&[String]>) -> Result<Found<Loc>, Error>;
pub struct Loc { pub heading_path: Vec<String>, pub paragraph: u32, pub kind: BlockKind }   // label()；Serialize
pub enum Error { …, NoHeading }                                                              // kind() = "no_heading"
pub mod tools { pub fn toolkit() -> Toolkit<()>; pub fn manifest() -> Vec<ToolSpec>; pub fn call(name, args, ctx) -> Result<Output, ToolError>; }
pub use engine_core;                                                                         // App 用同一份 engine-core
// BlockKind 加了 Serialize / Deserialize（snake_case）
```

- 命令行 `src/bin/engine-web.rs` 交回 `engine_core::cli::run`。
- `Toolkit` 的状态是 `()`：v0.2 的两个工具不需要 App 设置；v0.3 的抓取设置（User-Agent、代理……）会放进状态，到时 `toolkit()` 要改签名。
- 两个工具的参数、结果、出错、例子：`tools/*.json`；接线说明：`docs/toolkit.md`。
- `not_web` 的说明和建议在代码里按类型写（「这是 PDF，不是网页。」「请把这份文件交给 PDF 工具读。」），清单里那条是通用的。别的错误只交种类，说明照清单补。
- `web_extract` 分段按字数（`max_chars` 默认 2 万、`offset`），尽量断在空行或行尾；`next` 是接着取的参数。

## 测试

在云端 Linux 上跑的（不是 Mac）：

```bash
./scripts/check.sh
```

通过：`cargo fmt --check`；`cargo test`：单元 26、`tests/ported.rs` 6、`tests/samples.rs` 3（36 份样本，两种模式各跑一遍）、`tests/toolkit.rs` 6（清单自检、两个工具的 18 个例子、分段接起来等于全文、覆盖和导出、命令行 `tools` / `describe` / `call` 的 JSON 和退出码）；`cargo clippy --all-targets -- -D warnings`。

手动跑过：`cargo run --example pages`（GitHub 6 页，逐页看过）；`engine-web call web_find` 在 GitHub 的 `design.md` 快照上找了 4 句（整句 3 句、按词 1 句，位置都对）。

没跑：Mac 上的 `./scripts/check.sh`；GitHub 以外的真实网页（被拦）。

## 已知问题

- Phase 3 只核对了 GitHub 的 6 页，新闻、博客、文档站、知网、数据网站、GBK 老站都还没看。
- 正文不到 200 字时退回整个 `<body>`（Peck 的规则）：用 `<div>` 写导航、又没有 `role` 的短页，导航文字还会进正文。
- `article` 模式的表格是 dom_query 转的：跨行跨列的表头会拍成一行、和下面的列对不齐（字都在）。`full` 模式没这个问题。
- Readability 清掉了代码块的语言标记；代码块末尾多一个空行。
- `full` 模式照 Pith 去掉每行首尾空白（正文里代码缩进没了，分段里留着）；全角空格在行中折成一个空格。
- 作者那一行（「来源：示例日报 记者 林晓雨」）会留在正文开头。
- 响应式布局靠 CSS 隐藏的重复内容去不掉（GitHub 文件表）。

## 等站主

1. **放行 Phase 3 要抓的主机**（上面那 20 个）。在云端会话标题栏的环境菜单里编辑环境 → Network access：在「Allowed domains」里加这些主机，「Allow package managers」保持勾选（步骤见 <https://code.claude.com/docs/en/cloud-environments#network-access>）。Default 环境是所有仓库共用的，更合适的是新建一个只给 engine-web 用的环境，再用它开会话。放行以后跟云端说一声，云端用 `scripts/snapshot.sh` 抓、跑 `examples/pages`、补规则和样本。
2. **本机 Claude 审核、合并**：

   ```bash
   git fetch origin cloud/v0.2
   ```

   看 diff（重点：上面的公共接口、`tools/*.json` 的说明和例子、Phase 3 的 4 条新规则），在 Mac 上跑 `./scripts/check.sh` 和 `cargo run --bin engine-web -- tools`，过了 merge 进 `main`。

## 真实网页

快照只放 `local/`，不进 Git：

1. 网址写进 `local/pages/urls.txt`，一行一个（`#` 开头是注释）。
2. 抓快照（每个存成 `NNN-主机.html`，旁边 `.meta.json` 记最后的网址、Content-Type、日期；非 2xx 不存，已有的编号跳过，重跑只补没抓到的）：

   ```bash
   ./scripts/snapshot.sh
   ```

3. 跑核对，看 `local/out/summary.md` 和每页的 `*.article.md`、`*.full.txt`：

   ```bash
   cargo run --release --example pages
   ```

4. 把导航当正文、丢表格或链接、乱码的，记进本文件，每个问题做一份合成样本放进 `tests/fixtures/html/`，改规则。

## 下一轮

- 云端：站主放行主机后补完 Phase 3（GitHub 以外的网站），问题回到 `cloud/v0.2` 补规则和样本。
- 本机：审核合并 `cloud/v0.2`；在 Mac 上再过一遍真实网页；Phase 5 收尾（README、`CHANGELOG.md`、打 tag `v0.2.0`）。
