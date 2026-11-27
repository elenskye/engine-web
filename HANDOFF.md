# HANDOFF

更新：2026-11-27 · **v0.3.0 已发**（tag `v0.3.0`）· 本机 Claude Code

## 现在在哪

v0.3.0 能用了：取正文（v0.2）、**抓取**（`fetch`：网址 → 字节）、**Tavily**（搜索、取页、爬站、站内地图，本地报价、按回复记点数），六个工具 `web_extract`、`web_find`、`web_fetch`、`web_search`、`web_crawl`、`web_map` 和命令行。接口见 `CHANGELOG.md`，接线见 `docs/toolkit.md`，选型 `docs/decisions/0006`、`0007`。还没有 App 接。

## 这一轮（本机：v0.3.0 全部 + 修提交作者）

- **提交作者**：云端 `cloud/v0.2` 的 9 笔提交作者是 `Claude <noreply@anthropic.com>`。没推过的 `main` 上用 `git filter-branch` 改成了仓库配好的作者（内容、日期不变，GitHub 上的 `b0511dd` 没动，推 `main` 仍是快进）；没推过的 tag `v0.2.0` 跟着移到了新提交。`CLAUDE.md`「六」给云端加了「提交前先设作者」。
- **Phase 1 抓取**：`src/transport.rs`（`Transport`、默认的 `Ureq`：Mac 系统 TLS / 别的平台 rustls、解压 gzip br、只连钉住的地址、读到上限就停、逐块查取消）；`src/fetch.rs`（网址检查、内网检查、自己跟跳转、同站、https 降成 http 不跟、上限、跟到别的网站不带 Cookie、`HostPacer`）；`src/fake.rs`（feature `fake` 的模拟服务）。
- **Phase 2 Tavily**：`src/tavily.rs`。参数、计点、错误码照官方文档（2026-11-27 核对）。
- **Phase 3 工具包**：`src/tools/` 拆成 `mod.rs`（`Web`、`toolkit_with`、共用的错误和用量）、`extract.rs`、`fetch.rs`、`search.rs`；四份新清单，`web_find` 可只给网址。
- **Phase 4 真实核对**：
  - 本机抓 `docs/real-pages.txt` 的 72 个网址：58 个读到；没读到的 14 个是网站挡程序（澎湃、Stack Overflow 403）、要开 JavaScript（知网海外站、百度百科）、要登录或 cookie 跳转（万方会员、Nature 跳到 idp.nature.com）、证书对不上（kns / scjg.cnki.net）、页面撤了（统计局那篇 404）、服务挂了（古腾堡 503）。共 80 秒。
  - 查出 2 处已修，各配了样本或测试：react.dev 看到 `Accept: text/markdown` 回 MDX 源文件（Content-Type 是 text/plain），被认成网页、正文乱、标题错 → 开头是 YAML 头的认作 Markdown，标题取 `title`（样本 `mdx-lesson`）；教育部 https 跳到 http 被说成「跳到别的网站」→ 新的 `InsecureRedirect`，说清是降成了 http（`tests/fetch.rs`）。
  - 真跑 Tavily（`tests/real.rs` 和命令行）：搜索 basic 1 点、advanced + 新闻 2 点；取页、爬站 3 页、地图都在报价以内（不足一组时 Tavily 报 0 点、攒着下次算）；`web_fetch` 的 `via: auto` 在本机被 403 挡住时改用 Tavily 取到了 Stack Overflow 的正文。一共约 6 点（$0.05）。
- **密钥**：站主的 Tavily 密钥从剪贴板存进了 `~/Documents/Dataset/Pomelo/key/tavily`（600），没有打印。

## 测试

- `./scripts/check.sh`：**通过**（Mac）。`cargo fmt --check`；`cargo test` 和 `cargo test --features fake`；两种配置的 `cargo clippy --all-targets -D warnings`。带 `fake` 一共 79 个：单元 41、`tests/fetch.rs` 8、`tests/ureq.rs` 4（本机起 HTTP 服务）、`tests/tavily.rs` 4、`tests/tools_net.rs` 7（六个工具的全部清单例子、报价、确认、用量、命令行）、`tests/toolkit.rs` 6、`tests/samples.rs` 3（50 份样本）、`tests/ported.rs` 6。
- 真跑：`tests/real.rs` 2 个（`#[ignore]`），照下面「真跑」跑过，通过。

## 已知问题（不挡发版）

- Tavily 的站内地图在有的站上很浅（Rust 博客首页只找到 3 个网址，换参数一样），是 Tavily 那边的；文档站正常（Tavily 文档 21 个）。
- Nature 这类先跳到别的子域名设 cookie 再跳回来的，默认停在第一跳（同站只认「只差 `www.`」）。用户自己给的网址，App 可以放开 `follow_other_sites`；模型调用时可以换 `via: tavily`。
- Tavily 取页没有标题（交回的 Markdown 没有一级标题时结果里就没有「标题」一行）；代码块被 Tavily 拍成一行。
- 取正文的老问题照旧（见 `CHANGELOG.md` v0.2.0）：门户页退回整个 body 后 `<div>` 写的导航还在、`article` 模式跨行跨列的表头对不齐、`full` 模式把外框的 `<h*>` 当小标题、带站名后缀的标题照原样。
- 和 Peck、Pith、Pomelo 旧实现的逐字对照在各 App 接入时做（design §7）。

## 等站主

1. **push**（在 `engine-web` 目录里，`main` 先、tag 后）：

   ```bash
   git push origin main
   ```

   ```bash
   git push origin v0.2.0 v0.3.0
   ```

2. **删云端分支**（`main` 推上去以后；上面是改作者之前的旧提交）：

   ```bash
   git push origin --delete cloud/v0.2
   ```

3. **两样本机的残留可以清**（都只在本机、没有内容）：`git stash drop`（Mac 上文件名撞车留下的假改动）；`git update-ref -d refs/original/refs/heads/main` 和 `git update-ref -d refs/original/refs/tags/v0.2.0`（改作者时 git 自动留的旧指针）。
4. **剪贴板**里还有 Tavily 密钥，方便时复制点别的盖掉。

## 真跑

密钥只在验收里用：从站主的密钥文件夹复制到会话临时目录里 600 权限的文件，用环境变量交给 `#[ignore]` 的测试。不打印、不写进文档、夹具、日志和错误信息。每版上限 $1。

```bash
install -m 600 ~/Documents/Dataset/Pomelo/key/tavily <会话临时目录>/tavily.key
```

```bash
ENGINE_WEB_TAVILY_KEY_FILE=<会话临时目录>/tavily.key cargo test --release --test real -- --ignored --nocapture
```

命令行：`ENGINE_KEY_TAVILY="$(cat <会话临时目录>/tavily.key)" cargo run --bin engine-web -- call web_search '{"query":"…"}' --yes`。

## 下一轮

- v0.4.0 接 Peck：在 Peck 的仓库里做（守 Peck 的 `CLAUDE.md`），开工前问站主程度。要换的是 `harness/tools/web.rs` 的抓取和取正文、`gateway/search.rs` 的 Tavily；WebKit 渲染、一个任务内的缓存、原生搜索的路由留在 Peck。
