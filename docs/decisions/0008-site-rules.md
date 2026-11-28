# 0008 站点规则、文献信息、真实页

2026-11-28，v0.4 Phase 1–4。范围由站主定（2026-11-28 三次提问、一次对话），做法由本机 Claude 定。

## 范围（站主定）

- 站点：文献页（arXiv 摘要页、期刊文章页、工作论文和智库的文献页）、维基百科、GitHub 的 README 和 issue、微信公众号文章；文章里的图；多页文章自动合并。
- **知网、万方、维普移出 v0.4.0**：程序拿不到真实页（知网在本机网络下证书对不上、海外站要验证码；万方文献页是脚本外壳；维普文献页跳回首页），站主定中国大陆有网络问题的网站不抓、不作样本。以后能拿到真实页（站主从浏览器存，或 App 用 WebKit 渲染后交给 `web_extract`）再做。
- **认出挡程序的验证页**（站主 2026-11-28 在 Phase 3–4 的对话里同意，原计划放 Phase 5，提前做了）：认出来就报 `bot_check`，让模型改用 Tavily。
- **GitHub 长 issue 中间没放进网页的回复**（站主 2026-11-28 提问定）：写明缺几条、给出 GitHub 接口的网址，模型要看就自己用 `web_fetch` 取；引擎不另调接口。
- **PubMed 同样不做**：2026-11-28 起对一切程序回「要开 cookie」的脚本验证页，引擎、curl 都一样。同一批文献在 PMC（`pmc.ncbi.nlm.nih.gov`）上有全文和完整的 `citation_*` 元数据，走通用的文献信息。

## 站点规则怎么写

- **每站一个 Rust 模块**（`src/html/sites/`），交一份 `Site`：`key`（交给 `Article::rule`）、`matches`（认站，只看网址）、`fix`（两种模式都做，只改写法、不删正文）、`root`（正文的根，依次试）、`drop`（去掉的站点噪声）、`shape`（选好根以后再整理）；Phase 3 加了 `build`（正文在网页的 JSON 数据块里时从那里拼 HTML，拼不出再照 `root`）和 `meta`（站点自己写的标题、作者、日期、站名，两种模式都盖过通用读到的）。没写的项用 `..BLANK`。没选声明式的选择器表：几个站都要一两处代码（arXiv 的「标签 | 值」表、公众号放出藏起来的正文），表格写不下，写下了也难测。
- **选不到就退回通用做法**：认不出站、`root` 都选不到、根里不到 20 个字（网站改版），都照 v0.3 的做法抽。所以规则只会更好，不会更差。
- **只读 HTML**，包括网页里的 JSON 数据块（不执行脚本）；不另调网站的 API。存好的网页、App 用 WebKit 渲染后的 HTML 交给 `web_extract` 也用得上规则（要给原来的网址）。真实页证明 HTML 不够的站，在这里补例外。
- **规则只管自己的站**：没用上规则、也没有文献信息的网页，正文和 v0.3.0 逐字一样。Phase 1 在 50 份合成样本、65 页旧的真实页上核过一字不差；Phase 2 后变了的只有 3 页（旧的 arXiv 摘要页、Nature 用上规则，斯坦福哲学百科有 `citation_*`），`full` 模式的正文三页都没变。

## 文献信息（`Article::paper`）

- **通用的元数据**：照 Google Scholar 收录要求的 `citation_*`（Highwire Press）为主，PRISM（`prism.*`）、Dublin Core（`dc.*`、`dcterms.*`）补没写的。arXiv、PMC、Nature、PLOS、Frontiers、AEA、NBER、BIS、美联储、RePEc、Our World in Data 都有。
- **什么时候有**：有 `citation_title`，或者有 PRISM 的刊名加 DOI。只有 Dublin Core 的（政府、机构网页常用它标普通网页）不算。
- **不猜、不补**：照原样（去掉多余空白和标签），作者名的写法照网页（arXiv 写「Vaswani, Ashish」就是这样）；没写的空着。DOI 去掉 `doi:`、`https://doi.org/` 前缀。
- **标题**：有文献信息时，`Article::title` 用 `citation_title`（Frontiers 的 `<title>` 只写「Frontiers」）。作者、日期在通用的读不到时用文献信息补。
- **参考文献的查找链接**：有文献信息的网页，`article` 模式去掉参考文献后面的 Google Scholar、Nature 的 CAS 链接（不带内容，Nature 一页 127 个），只剩这个链接的列表项一起去掉；参考文献本身、DOI 和 PubMed 的链接留着。Nature、PMC、PLOS、Frontiers 的正文因此少一到两成。`full` 模式不动（核对原文要全部文字）。
- **给模型的**：工具结果在标题下面列「文献」几行（作者和机构、刊名卷期页、日期、DOI、编号、PDF、关键词）；摘要不在这里重复，正文里本来就有。结构化的在 `data.paper`。

## 挡程序的验证页（`Article::bot_check`，Phase 3–4）

- **只看正文很短的页**（不算空白不到 1500 字）：长文章里提到这些字、普通网页里 Cloudflare 塞的脚本都不算。
- **先看各家验证页特有的写法**（`window._cf_chl_opt`、`/cdn-cgi/challenge-platform/h/`、Fastly 的 `/_fs-ch-`、Imperva、DataDome、PerimeterX、AWS WAF、Akamai、Anubis、Vercel、公众号的 `secitptpage/verify`、PubMed 算工作量证明的脚本），记下是哪一家；**再看标题和正文**里的几句（「Just a moment...」「Checking your browser」「Cookies must be enabled」「安全验证」「环境异常」……），认不出哪家的记 `unknown`。
- **单页应用要开 JavaScript 不算**（那是 `no_article`）：「You need to enable JavaScript to run this app」这种不在里面。
- **工具**：`web_extract`、`web_fetch`、`web_find` 报 `bot_check`（说明写是哪一家，下一步换 `via: tavily`）；`via: auto` 见到它会改用 Tavily；多页时中途哪一页是验证页就停在那里。`extract` 本身不报错，App 自己看 `bot_check`。
- 核过：63 页旧真实页、60 页 v0.4 真实页里认出了全部 8 页验证页（PubMed 4 页、BMC、AnandTech 2 页、知网的「安全验证」），公众号的验证页另存一份也认出来了；别的页一页没认错。

## 文中的图（`Article::images`，Phase 4）

- **从转成正文的那份 HTML 里找**（规则选的根、Readability 抽出的、退回的整个 `<body>`；`full` 模式是整页）：网址、替代文字、图注（`figcaption`，维基的 `.thumbcaption`、信息框的 `.infobox-caption`，WordPress 的 `.wp-caption-text`）、在第几段后面。
- **网址**：`srcset` 取最大的（`500w`、`2x`），懒加载的取 `data-src` 这类，`src` 是占位图（`data:`、`blank.gif`）时不用；补成绝对网址。
- **不要的**：图标（宽高都写了、都不过 48）、1×1 的统计图、表情、头像、徽章（shields.io、`badge.svg`）；同一张只留第一次。
- **第几段**：图前面那 24 个字（只比字母和数字）在分段里找，找到的那一段就是「图前面最近的一段」；在正文最前面的是 `None`。**不改正文**：不往 HTML 里插记号，Markdown 和分段跟没有这一步时一字不差（合成样本和真实页都核过）。
- **给模型的**：工具结果第一段的末尾列「文中的图」前 20 张（第几段后、说明、网址），全部在 `data.images`；要看哪张就用 `web_fetch` 取原件（已经能用）。

## 多页（Phase 4）

- **认下一页**（`Article::next_page`）：先看 `<link rel="next">`，再看正文里的链接：`rel="next"` > 「下一页」「Next」「次のページ」「Nächste Seite」这类字 > 页码「2」（当前页加一）。不管哪来的，网址都要像这一页的下一页：同一主机、去掉页号以后一样、页号加一（`?page=2`、`/page/2`、`/seite-2`、`news008_2.html`、`/2/`、`index2.html`，第一页可以不带页号）；光秃秃的数字只认到 50（再大多半是文章编号）。所以「下一篇」（别名、日期、编号不一样）不跟。
- **接着抓**（`follow`、`fetch_article`，`web_fetch`、`web_find` 的 `max_pages`，默认 10、最多 20、1 就不抓）：只走本机抓取（`HostPacer` 照样管间隔）；抓过的网址、和上一页一样的正文就停；每页之前查取消（取消就整个报 `cancelled`）；中途哪一页没取到（出错、验证页、没有正文、不是网页）交回前面几页，`stopped` 写明哪一页为什么；到了上限写明后面还有哪一页。**列表页不跟**：链接的字占了正文一半以上（首页、目录、搜索结果；Hacker News 是 73%，文章页都在 3% 以下），那里的「下一页」是更多条目，只在结果里写出它的网址。`via: tavily` 不合并（Tavily 交的 Markdown 认不出下一页）。
- **合成**：正文每页之间隔一行「第 N 页：网址」；标题、作者这些用第一页的；分段、图都记第几页；链接、每页都有的图去重。
- **位置**：第几页 + 这一页里的标题路径和第几段，和单独抓那一页时一样，能逐页核对；写法「第 2 页〈方法〉第 3 段」，只有一页时不写页（`Block`、`Loc`、`Image` 的 `page` 只有合了几页时有）。

## 真实页

- Phase 3–4 加了 25 个网址（快照 047–071）：中文、手机版、公式多的维基百科，GitHub 的仓库首页、文件页、长短 issue，公众号文章 5 篇，分页的长文（@IT 的 `_2.html`、ComputerBase 的 `/seite-2`、Discourse 论坛的 `?page=2`）。
- 公众号（中国大陆的网站）本机抓得到，但会时不时回验证页（同一篇文章第一次回验证页、过一会儿又能抓到）：规则在抓到的 4 篇上核过，验证页由 `bot_check` 认出来、交给 Tavily。
- 只用国际上程序拿得到的（站主 2026-11-28 定）：快照在 `local/pages/sites/`（不进 Git），网址和抓不到的站列在 `docs/real-pages-sites.txt`（复制到 `local/pages/sites/urls.txt` 就能重抓）。
- 挡程序、不作样本的（2026-11-28）：PubMed（cookie 验证）、eLife（脚本验证）、BMC 和 AnandTech（浏览器验证页）、MDPI、Science、ScienceDirect、PeerJ、英国皇家学会、Phoronix、世界银行数据页（403）、Ars Technica（405）、Springer Link（跳到 `idp.springer.com` 设 cookie）、FRED（HTTP/2 断开）、Eurostat（500）。

## Phase 2 的规则（2026-11-28，真实页核过）

| 规则 | 认站 | 做什么 | 核过的真实页 |
| --- | --- | --- | --- |
| `arxiv` | `arxiv.org/abs/…` | 根 `#abs-outer`；去掉「Title:」「Abstract:」标签和题名行、作者去链接、「标签 \| 值」表改成列表、全文链接留一行、提交记录一版一行；右栏、arXivLabs、推荐人、给手机的下载按钮去掉 | 4 页（新旧两种编号、带期刊引用和相关 DOI 的） |
| `plos` | `journals.plos.org/…/article` | 根 `#artText`（从摘要起）；去掉图下面的下载链接、空锚点（不然参考文献的序号和作者连在一起） | 2 页 |
| `aea` | `www.aeaweb.org/articles` | 根是文章那一栏；去掉期刊页头、菜单、题名行、页内跳转 | 1 页 |
| `nature` | `www.nature.com/articles/…` | 没有根（通用做法已经好）；去掉「Similar content」、分享框、读者评论 | 2 页（新旧快照） |

只靠通用文献信息、不用规则的：PMC、Frontiers、NBER、BIS、美联储 FEDS Notes、RePEc、Our World in Data、斯坦福哲学百科。

## Phase 3 的规则（2026-11-28，真实页核过）

| 规则 | 认站 | 做什么 | 核过的真实页 |
| --- | --- | --- | --- |
| `wikipedia` | `*.wikipedia.org/wiki/…`（手机版、中文的 `/zh-cn/…`、`/w/index.php` 也算） | 根 `#mw-content-text > .mw-parser-output`；信息框读成两列的表（框名、开头只有一格的行挪到表前，表里的图挪到表前、小图标去掉）；引用角标留成「[1]」文字，参考文献每条前面写同样的角标（注释组「[a]」也对得上）；红链只留字；表格正文行的 `<th>` 当普通格子；公式写成 `$TeX$`（Parsoid 的 `data-mw`）；去掉 edit 链接、导航框、侧栏、「其他用法」提示、维护模板、目录、坐标、回链；图注单成一段 | 英、德、日、中文各一页以上，手机版，公式多的一页；旧的 3 页（中文、英文、长表格） |
| `github` | `github.com/<owner>/<repo>…` | README、Markdown 文件：根 `article.markdown-body`（去掉小标题锚点）；issue：从 `react-app.embeddedData` 拼状态、标签、里程碑、关联的 PR、提问和每条回复（作者、身份、时间），关闭 / 重开一行，被折叠的只写理由，中间没放进网页的写明几条、给接口网址；代码文件：`rawLines` 拼成一段代码。标题「仓库 / 文件」「标题 · 仓库#号」 | 仓库首页 2 个、文件页 1 个、issue 4 个（114 条的长帖、短帖、带代码的）；旧的 7 页（首页、文件页、目录、提交记录、代码文件） |
| `weixin` | `mp.weixin.qq.com/s/…`、`/s?__biz=…` | `fix` 去掉 `#js_content` 的 `visibility: hidden`（两种模式）；根 `#js_content`；标题、作者、公众号名从页头读，发布时间从脚本变量 `ct` 按北京时间写 | 4 篇（法律、学报、宣传号、日程）；「账号已迁移」的一篇没有正文，照通用做法交回那句话 |

规则以外的变化（图、下一页、验证页）不改正文：Phase 3–4 后，旧真实页 63 页、v0.4 真实页 60 页、旧合成样本 42 份里没用上新规则的，`article`、`full` 两种模式的结果都和改之前一字不差；用上规则的（维基百科、GitHub）`full` 模式的正文也没变，只变了标题（GitHub）。
