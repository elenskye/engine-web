# HANDOFF

更新：2026-11-27 · v0.2.0 Phase 1–2 · 云端 Claude（分支 `cloud/v0.2`）

## 现在在哪

v0.2.0 的 Phase 1（骨架与样本）、Phase 2（取正文）做完，在 `cloud/v0.2` 上，**没在 Mac 上验**。等本机审核、合并，再做 Phase 3（真实网页核对）。Phase 4（工具包与 find）等 engine-core 的 `v0.2.0` tag（现在 engine-core 还没有 tag）。

这一轮的 commit（都在 `cloud/v0.2`，最后一笔见 `git log`）：

| commit | 说明 | 做了什么 |
| --- | --- | --- |
| `5c80368` | `v0.2.0: phase 1 types, samples and pages example` | `Error`、`Article` 等类型；合成样本和期望；`examples/pages`、`examples/make_samples`；版本改成 0.2.0 |
| `725670f` | `v0.2.0: phase 2 extraction, encoding and type detection` | 类型判断、编码、`article` / `full` 两种模式、分段、元数据；搬来的 Peck、Pith 测试 |
| 最后一笔 | `v0.2.0: chardetng, decisions and handoff` | 编码探测换成 chardetng；决定 0002–0005；设计稿、README、ROADMAP；`scripts/snapshot.sh` |

这一轮站主在提问里定的：云端分支用 `cloud/v0.2`；HTML 依赖跟 Peck 同一版（`docs/decisions/0002`）；`full` 模式搬 Pith 的走树（0003）；段号全文连续、小标题也算一段、路径不含文章标题（0004）；没声明编码时用 chardetng（0005）。

## 改了哪些文件

- `src/`：`lib.rs`（`extract`）、`article.rs`（结果类型）、`error.rs`（`Error`、`Binary`）、`sniff.rs`（类型）、`decode.rs`（编码）、`html.rs`（两种模式、元数据、链接）、`blocks.rs`（分段、标题路径）。
- `tests/`：`samples.rs`（读样本对期望）、`ported.rs`（Peck、Pith 搬来的）、`fixtures/html/`（34 份样本 + `.expect.json` + `README.md`）。
- `examples/`：`pages.rs`（批量核对）、`make_samples.rs`（生成非 UTF-8 和二进制样本）。
- `scripts/snapshot.sh`（新）：Phase 3 抓快照用。
- 文档：`docs/decisions/0002`–`0005`（新）、`docs/design.md`（§2 v0.2 接口、§3 编码和类型、§4、§6、§10）、`README.md`、`ROADMAP.md`、`THIRD_PARTY.md`、`Cargo.toml`。

## 公共接口（请本机审）

```rust
pub fn extract(bytes: &[u8], content_type: Option<&str>, url: &str, opts: &ExtractOptions) -> Result<Article, Error>;
pub fn sniff(bytes: &[u8], content_type: Option<&str>, url: &str) -> Result<Kind, Binary>;
pub fn decode(bytes: &[u8], content_type: Option<&str>) -> (String, &'static str);
pub struct ExtractOptions { mode: Mode, min_article_chars: usize }          // Default：Article、200
pub enum Mode { Article, Full }                                              // key()
pub enum Kind { Html, Markdown, Text, Json, Xml }                            // key()
pub struct Article { kind, encoding, title, byline, published, site, lang, markdown, blocks, links, needs_js }
pub struct Block { heading_path: Vec<String>, paragraph: u32, kind: BlockKind, text: String }  // label()
pub enum BlockKind { Heading, Paragraph, ListItem, TableRow, Code }          // key()
pub struct Link { text, url }
pub enum Error { BadUrl, PrivateAddress, RedirectElsewhere { to }, HttpStatus { status }, NotWeb(Binary),
                 NoArticle, TooLarge, Network, Timeout, Auth, Quota, RateLimited, Service, Cancelled }  // kind()
pub enum Binary { Pdf, Image, Word, Excel, PowerPoint, Epub, Archive, Audio, Video, Other }            // key()、label()
```

- 比 design §2 草案多的：`sniff`、`decode` 公开（Pith 接入时其他类型要用）；`Binary` 细分了 Office 和 EPUB（App 按它交给 engine-word / engine-excel 等）；`Block::label()`。
- `Error` 先把 design §1 的种类全定下来了，v0.2 的 `extract` 只交 `NotWeb`；`NoArticle` 留给工具层（正文为空时）。
- 正文为空不算错：交回空正文、空分段和 `needs_js`。空字节当空的纯文本。

## 和 Peck、Pith 旧行为的差别（有意的）

- 编码：合法的 UTF-8 排在声明前面（Pith 一样，Peck 只看 HTTP 头）；声明 UTF-8 而有坏字节的还按 UTF-8（Pith 会退 GB18030）；没声明的用 chardetng（`docs/decisions/0005`）。
- `article` 模式（对 Peck）：隐藏元素先去掉；路径里带 `:` 的相对链接也补全；dom_query 不需要的转义还原（`12\.6` → `12.6`）；标题多「唯一 h1 + 分隔符 + 站名 → h1」和「第一个 h1」两条退路。
- 类型（对 Pith 的 `sniff`）：CSV 认作纯文本；旧版 Office 按 UTF-16LE 的流名分 Word / Excel / PowerPoint（Pith 找 ASCII 的 `Workbook`，没有扩展名和 Content-Type 时认不出 .xls）；EPUB、MP4、WebP 等认得出；像文字但 Content-Type 说是图片的信 Content-Type（Peck 的测试要这样）；空文件不报错。
- 没搬的：Pith 的 60 万字上限（v0.2 Phase 4 的 `max_chars` / `offset` 在工具层做）；Peck 的 `check_url`、`is_public`、`same_site` 等抓取测试（v0.3 搬）。

## 测试

在云端 Linux 上跑的（不是 Mac）：

```bash
./scripts/check.sh
```

通过：`cargo fmt --check`；`cargo test`：单元 21、`tests/ported.rs` 6、`tests/samples.rs` 3（34 份样本，两种模式各跑一遍）；`cargo clippy --all-targets -- -D warnings`。

另外手动跑过：`cargo run --example pages -- tests/fixtures/html`（逐页看过输出）；`cargo run --example pages`（没有 `local/pages/` 时说「没有真实网页」）；`scripts/snapshot.sh` 对本机假服务器抓了 4 个网址、`examples/pages` 读得回来。

没跑：Mac 上的 `./scripts/check.sh`；任何真实网页。

## 已知问题（Phase 3 重点看）

- 正文不到 200 字时退回整个 `<body>`（Peck 的规则）：用 `<div>` 而不是 `<nav>`、`<footer>` 写导航的短页，导航文字会进正文。样本里的 GBK 页一开始就中了，加长才过。真实网页上要看多不多。
- `article` 模式的表格是 dom_query 转的：跨行跨列的表头（`rowspan`、`colspan`）会拍成一行、和下面的列对不齐（字都在）。`full` 模式没这个问题。
- Readability 清掉了代码块的语言标记（```` ```rust ```` 变成 ```` ``` ````）；代码块末尾多一个空行。
- `full` 模式照 Pith 去掉每行首尾空白，正文里代码的缩进没了（分段里的代码块留着缩进）；全角空格在行中折成一个空格。
- 作者那一行（「来源：示例日报 记者 林晓雨 2026-10-18」）会留在正文开头，Readability 没去。

## 真实网页

Phase 3 在本机做，快照只放 `local/`，不进 Git：

1. 站主把网址写进 `local/pages/urls.txt`，一行一个（`#` 开头的是注释），20–40 个，中英文、新闻、博客、文档站、带表格的、GBK 老站都要有。站主不方便的话，本机 Claude 自己挑一批公开页面。
2. 抓快照（每个存成 `NNN-主机.html`，旁边 `.meta.json` 记最后的网址、Content-Type、日期；非 2xx 不存）：

   ```bash
   ./scripts/snapshot.sh
   ```

3. 跑核对，看 `local/out/summary.md` 和每页的 `*.article.md`、`*.full.txt`：

   ```bash
   cargo run --example pages
   ```

4. 把导航当正文、丢表格或链接、乱码的，记进本文件，每个问题做一份合成样本放进 `tests/fixtures/html/`，回到 `cloud/v0.2` 改规则。

## 等站主

1. **本机 Claude 审核、合并**：

   ```bash
   git fetch origin cloud/v0.2
   ```

   看 diff（重点：上面的公共接口、和旧行为的差别），在 Mac 上跑 `./scripts/check.sh`，过了 merge 进 `main`。
2. **Phase 3 的网址清单**：见上面「真实网页」第 1 步。
3. **engine-core `v0.2.0` 的 tag**：Phase 4 要依赖它（`Toolkit`、`find`、命令行骨架）。

## 下一轮

- 本机：审核合并 `cloud/v0.2`，做 Phase 3。
- 云端：Phase 3 发现的问题回到 `cloud/v0.2` 补规则和样本；engine-core 发 `v0.2.0` 后做 Phase 4（`tools/web_extract.json`、`tools/web_find.json`、`Toolkit`、`find`、命令行、`docs/toolkit.md`）。
