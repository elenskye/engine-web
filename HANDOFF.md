# HANDOFF

更新：2026-11-27 · v0.1.0 计划 · 本机 Claude Code

> 准则更新（2026-11-27）：`CLAUDE.md`「二」末尾加了「改用到的引擎」（总仓库 `docs/decisions/0009`）。用到别的引擎就能直接改它、照它的规矩提交和打 tag，动之前读它的 `CLAUDE.md`；改它已有的接口先问站主。

## 现在在哪

v0.1.0：只有计划和骨架，没有可用的接口。

- **计划**：`ROADMAP.md`，版本表到 1.0，v0.2.0 的 Phase 表标明了云端和本机。
- **设计稿**：`docs/design.md`。三家现状、工具清单草案、Rust 接口草案、抓取、取正文、Tavily、find 的位置、迁移和核对、样本、分工。
- **接线说明草案**：`docs/toolkit.md`。
- **范围和分工**：`docs/decisions/0001`，云端以它为准。
- **骨架**：`Cargo.toml`、`src/lib.rs`（版本测试）、`scripts/check.sh`。

## 测试

- `./scripts/check.sh`：通过（`cargo fmt --check`、`cargo test` 1 个、`cargo clippy --all-targets -D warnings`）。

## 等站主

1. **GitHub**：建 `elenskye/engine-web`，加远端、push `main`（命令总仓库那边给）。
2. **云端账号**：给云端那个 Claude 账号开 `elenskye/engine-web` 的访问权限。跟它说：读 `CLAUDE.md`、`docs/decisions/0001`、`ROADMAP.md`，做 v0.2.0 标「云端」的 Phase，在 `cloud/v0.2` 分支上做，只推这条分支。
3. **看 v0.2.0 的计划**：`ROADMAP.md`「v0.2.0 取正文」。看过说一声就能开工。
4. **真实网页**（v0.2 Phase 3 用）：把三个 App 里常读的网页网址写进 `local/pages/urls.txt`，一行一个，20–40 个，中英文、新闻、博客、文档站、带表格的都要有。站主不方便的话，本机 Claude 自己挑一批公开页面。快照只放 `local/`，不进 Git。
5. **v0.3 真跑 Tavily 前**：确认沿用 Pith 那份 Tavily 密钥，每版上限 $1。

## 下一轮

- 站主批了 v0.2.0 计划后，云端 Claude 从 Phase 1 开始（`cloud/v0.2`）。
- Phase 4 等 engine-core 发 `v0.2.0`。
- 本机在 Phase 2 之后做 Phase 3（真实网页核对）。
