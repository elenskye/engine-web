# 工具包接线说明（草案）

> v0.1.0 草案，v0.2.0 Phase 4 补完。格式照 engine-core（`Toolkit`、`Ctx`、`Overrides`、`to_anthropic` / `to_openai`）。

## App 要交进来的

| 上下文 | engine-web 拿来做什么 |
| --- | --- |
| `cancel` | 抓取、爬站、Tavily 重试的等待中都查 |
| `keys` | `tavily`：Tavily 的 API 密钥（只在要调 Tavily 时取；没有就不给 `web_search`、`web_crawl`、`web_map`，`web_fetch` 不走 Tavily） |
| `files` | `web_extract`、`web_find` 读本机存好的网页原件；App 在这里管能读哪些路径 |
| `approve` | 爬站、地图的报价确认框（服务、页数上限、预计美元） |
| `prices` | Tavily 每点多少美元（默认 $0.008） |
| `progress` | 爬站、多页合并的进度 |

抓取用的网络设置（User-Agent、Cookie 等请求头、代理、能不能抓内网、跟不跟别的网站的跳转、超时和大小）由 App 在建 `Toolkit` 时交给引擎，不经模型。

## App 能覆盖的

照 engine-core 的 `Overrides`，按工具名：

- 改说明，比如 Peck 用英文说明；
- 改默认值：`max_results`、`mode`、`max_chars`、`via`；
- 收紧上限：`web_crawl` 的页数；
- 改确认方式：比如 App 让 `web_crawl` 在 $0.05 以下不问；
- 关掉不用的工具。

结果的格式不改。

## 三家大概怎么接

| App | 接哪些工具 | 要注意的 |
| --- | --- | --- |
| Peck | `web_search`（没有原生搜索的模型用）、`web_fetch`、`web_find` | 原生搜索的路由、DeepSeek 搜索、一个任务内的缓存、WebKit 渲染、存输出目录都留在 Peck；`needs_js` 为真时 Peck 照旧用 WebKit 渲染，再把渲染后的 HTML 交给 `extract` |
| Pomelo | `web_search`（插件库的联网搜索、Pub2MD 查术语）、`web_fetch`（「添加网页」） | 模拟模式换成 `FakeTransport`；「添加网页」是用户自己给的网址，可以跟别的网站的跳转 |
| Pith | `web_search`、`web_fetch`（`mode: full`，核对摘录用）、`web_find` | 月上限照旧在 Pith 查（用引擎的报价）；存快照、切块存库、Claude 网页抓取的后备留在 Pith |
