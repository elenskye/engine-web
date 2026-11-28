# 0008 站点规则、文献信息、真实页

2026-11-28，v0.4 Phase 1–2。范围由站主定（2026-11-28 两次提问），做法由本机 Claude 定。

## 范围（站主定）

- 站点：文献页（arXiv 摘要页、期刊文章页、工作论文和智库的文献页）、维基百科、GitHub 的 README 和 issue、微信公众号文章；文章里的图；多页文章自动合并。
- **知网、万方、维普移出 v0.4.0**：程序拿不到真实页（知网在本机网络下证书对不上、海外站要验证码；万方文献页是脚本外壳；维普文献页跳回首页），站主定中国大陆有网络问题的网站不抓、不作样本。以后能拿到真实页（站主从浏览器存，或 App 用 WebKit 渲染后交给 `web_extract`）再做。
- **PubMed 同样不做**：2026-11-28 起对一切程序回「要开 cookie」的脚本验证页，引擎、curl 都一样。同一批文献在 PMC（`pmc.ncbi.nlm.nih.gov`）上有全文和完整的 `citation_*` 元数据，走通用的文献信息。

## 站点规则怎么写

- **每站一个 Rust 模块**（`src/html/sites/`），交一份 `Site`：`key`（交给 `Article::rule`）、`matches`（认站，只看网址）、`fix`（两种模式都做，只改写法、不删正文）、`root`（正文的根，依次试）、`drop`（去掉的站点噪声）、`shape`（选好根以后再整理）。没选声明式的选择器表：几个站都要一两处代码（arXiv 的「标签 | 值」表、公众号放出藏起来的正文），表格写不下，写下了也难测。
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

## 多页的位置（Phase 4 实现）

第几页 + 这一页里的标题路径和第几段，和单独抓那一页时一样，能逐页核对；写法「第 2 页〈方法〉第 3 段」，只有一页时不写页。

## 真实页

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
