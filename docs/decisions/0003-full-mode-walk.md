# 0003 `full` 模式：搬 Pith 的走树，换成 dom_query

2026-11-27，v0.2 Phase 2，站主在提问里定的（design §10 的未定项）。

- `full` 模式照搬 Pith `research/content.rs` 的 `walk` + `tidy`，只把 `scraper` 换成 `dom_query`：全部文字、跳过脚本样式表单和隐藏元素、表格一行一行、格子用制表符隔开。
- 链接只留文字，网址进 `Article.links`；不写成 `[文字](网址)`。Pith 拿它核对摘录，正文里夹网址会对不上。
- 和 Pith 旧输出逐字一致：Pith 的 `html_becomes_readable_text_and_tables_become_rows` 原样搬进 `tests/ported.rs`。
- 照 Pith 的副作用也留着：每行去掉首尾空白（代码缩进在正文里没了，分段里的代码块保留缩进）；全角空格在行中折成一个空格。
- 没选 dom_query 的整页 Markdown：表格会变成 Markdown 表格、链接内联，和 Pith 旧行为差太多。
