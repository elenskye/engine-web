# 0007 Tavily 和联网工具

2026-11-27，v0.3 Phase 2–3，本机 Claude 定（照 Tavily 官方文档；design §1 的草案有两处改了，下面标「改」）。

## Tavily 的计点（官方文档，2026-11-27 核对）

| 接口 | 点数 |
| --- | --- |
| 搜索 `/search` | basic 1 点，advanced 2 点 |
| 取页 `/extract` | 每 5 个成功的网址 1 点（advanced 2 点）；失败的不收 |
| 站内地图 `/map` | 每 10 页 1 点（写了 `instructions` 2 点） |
| 爬站 `/crawl` | 地图的点数 + 取页的点数（官方例子：10 页 basic 3 点、advanced 5 点） |

按量付费每点 $0.008（App 用 `Prices` 改）。每个请求带 `include_usage: true`，按回复的 `usage.credits` 记；回复里没有就按报价记、标「估算」。Tavily 不足一组时可能报 0 点（攒着下次算），照实记。

## 报价、确认、上限

- **报价按最坏情况、不联网**：不足 5 或 10 的按一组；爬站、地图按 `limit` 算。
- **确认**：搜索、取页不问（单次最多 2 点）；爬站、地图每次问（`always`）。命令行对一切正金额都要 `--yes`（engine-core 的规矩）。（2026-11-28 站主改了规矩：一次超过 $0.5 或算不出、24 小时合计超过 $5 才问，见 `CLAUDE.md`「五」；v0.4.0 起代码照此改了：`web_crawl`、`web_map` 改成 `never`，由 engine-core 的 `spend` 照账本定。）
- **上限**：爬站最多 50 页、往下 3 层（最多 30 点）；地图最多 200 个网址（最多 40 点）。爬站、地图不出这个网站（`allow_external: false`，Tavily 默认是 true）。
- **出错**：401 → `auth`；403 搜索、取页是 `auth`，爬站、地图是 `unsupported_site`（文档：网址不支持）；400、422 → `bad_args`；432、433 → `quota`；429、434 → `rate_limited`；5xx → `service`。429、434、5xx 按 `Retry-After` 等了再试，最多 3 次，等的时候可以取消。服务原文不交出去。

## 联网工具

- **改：`web_fetch` 的 `via` 默认 `local`**（草案是 `auto`）。engine-core 的报价函数看不到密钥，`auto` 只能按要花钱报价，那命令行每次读网页都得加 `--yes`；而且花钱应当是明着选的。现在：`local` 报价 0；`tavily`、`auto` 按取一页报价。App 想默认 `auto` 用覆盖改默认值就行。`auto` 只在取不到正文、被网站挡（401、403、429）、连不上、超时时改用 Tavily，内网、网址不对、跳到别的网站不换；没有密钥就不换。
- **改：PDF、图片这类不报错**，原件作附件交回（`not_web` 写进 `data`，说明交给哪个工具），App 拿附件交给 engine-pdf 等。所以 `web_fetch` 的影响是 `new_bytes`。
- **`web_find` 可以只给 `url`**：本机现抓一次（免费），`effects.network` 改成 true。
- **`toolkit()` 的类型改成 `Toolkit<Web>`**：`Web` 是抓取设置、`Transport`、Tavily 网址；`toolkit_with(Web)` 交 App 自己的。
