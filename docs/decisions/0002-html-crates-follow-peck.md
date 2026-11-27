# 0002 HTML 依赖跟 Peck 同一版

2026-11-27，v0.2 Phase 1，站主在提问里定的。

- `dom_smoothie ^0.18.2`、`dom_query ^0.28.0`，和 Peck 现在钉的一样（Peck 和 Tauri 共用 html5ever 0.39）。
- 更新的 0.20 / 0.30 用 html5ever 0.40：Peck 接入时会多编一份 html5ever，搬来的 `html_to_markdown` 输出也可能变。
- 以后要升，三家（Peck、Pomelo 和本引擎）一起升，在引擎的一个版本里做，样本和真实网页再核对一遍。
