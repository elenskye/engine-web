//! `full` 模式：Pith 的 `html_text`（换成 dom_query）。不抽正文，整棵树看得见的文字都要；
//! 表格一行一行、格子用制表符隔开；核对摘录用。文字（`walk`）和分段（`Blocks`）各走一遍树，跳过的元素一样。

use super::clean::pre_text;
use super::meta::{Meta, title_of};
use super::{BLOCK, Parts, SKIP, hidden, links_in, name_of, needs_js};
use crate::BlockKind;
use crate::blocks::{self, Raw};
use dom_query::{Document, NodeRef};
use url::Url;

pub(super) fn extract(html: &str, base: Option<&Url>) -> Parts {
    let doc = Document::from(html);
    let root = doc.select("html");
    let root = root.nodes().first().cloned().unwrap_or_else(|| doc.root());
    let mut raw = String::new();
    walk(&root, &mut raw, false);
    let markdown = tidy(&raw);
    let mut walker = Blocks::default();
    walker.walk(&root, false, BlockKind::Paragraph);
    walker.flush();
    let meta = Meta::read(html, base.map(Url::as_str));
    let title = title_of(&meta.title, meta.site.as_deref(), &doc);
    let blocks = blocks::number(walker.out, title.as_deref());
    let links = links_in(&root, base, SKIP);
    let needs_js = needs_js(&markdown, html);
    meta.into_parts(title, markdown, blocks, links, needs_js)
}

/// HTML 的空白折叠：连续的空白当一个空格；开头、换行和制表符后面不加。
fn push_collapsed(out: &mut String, text: &str) {
    let mut last_space = out.is_empty() || out.ends_with([' ', '\n', '\t']);
    for c in text.chars() {
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(c);
            last_space = false;
        }
    }
}

/// 走树取全部文字（Pith 的 `walk`）：跳过脚本、样式、表单和隐藏的元素；块级元素前后换行；表格一行一行，格子用制表符隔开。
fn walk(node: &NodeRef, out: &mut String, in_pre: bool) {
    for child in node.children() {
        if child.is_text() {
            let t = child.text();
            if in_pre {
                out.push_str(&t);
            } else {
                push_collapsed(out, &t);
            }
            continue;
        }
        if !child.is_element() {
            continue;
        }
        let name = name_of(&child);
        if SKIP.contains(&name.as_str()) || hidden(&child) {
            continue;
        }
        match name.as_str() {
            "br" => out.push('\n'),
            "td" | "th" => {
                // 一行里第二格起，前面加制表符
                if !out.ends_with('\n') && !out.is_empty() {
                    let line_start = out.rfind('\n').map(|i| i + 1).unwrap_or(0);
                    if !out[line_start..].trim().is_empty() || out.ends_with('\t') {
                        out.push('\t');
                    }
                }
                let mut cell = String::new();
                walk(&child, &mut cell, false);
                // 单元格里的换行会把一行表格拆散：换成空格
                out.push_str(&collapse(&cell));
            }
            _ => {
                let block = BLOCK.contains(&name.as_str());
                if block && !out.ends_with('\n') && !out.is_empty() {
                    out.push('\n');
                }
                if matches!(
                    name.as_str(),
                    "p" | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "table"
                        | "blockquote"
                        | "pre"
                        | "ul"
                        | "ol"
                ) && !out.ends_with("\n\n")
                    && !out.is_empty()
                {
                    out.push('\n');
                }
                walk(&child, out, in_pre || name == "pre");
                if block && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
    }
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn trim_line(line: &str) -> &str {
    line.trim_matches(|c: char| c == ' ' || c == '\u{00A0}' || c == '\u{3000}' || c == '\r')
}

/// 整理空白：每行去掉首尾空白，连续的空行并成一个（Pith 的 `tidy`）。
fn tidy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank = 0;
    for line in text.lines() {
        let line = trim_line(line);
        if line.is_empty() {
            blank += 1;
            continue;
        }
        if !out.is_empty() {
            out.push_str(if blank > 0 { "\n\n" } else { "\n" });
        }
        blank = 0;
        out.push_str(line);
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// 分段：和 `walk` 跳过同样的元素，按块级元素切段。
#[derive(Default)]
struct Blocks {
    out: Vec<Raw>,
    buf: String,
    kind: Option<BlockKind>,
}

impl Blocks {
    fn flush(&mut self) {
        let text = self
            .buf
            .lines()
            .map(trim_line)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            self.out
                .push(Raw::new(self.kind.unwrap_or(BlockKind::Paragraph), text));
        }
        self.buf.clear();
        self.kind = None;
    }

    fn push_text(&mut self, t: &str, in_pre: bool, kind: BlockKind) {
        if self.kind.is_none() && !t.trim().is_empty() {
            self.kind = Some(kind);
        }
        if in_pre {
            self.buf.push_str(t);
        } else {
            push_collapsed(&mut self.buf, t);
        }
    }

    fn walk(&mut self, node: &NodeRef, in_pre: bool, kind: BlockKind) {
        for child in node.children() {
            if child.is_text() {
                self.push_text(&child.text(), in_pre, kind);
                continue;
            }
            if !child.is_element() {
                continue;
            }
            let name = name_of(&child);
            if SKIP.contains(&name.as_str()) || hidden(&child) {
                continue;
            }
            match name.as_str() {
                "br" => self.buf.push('\n'),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    self.flush();
                    let text = collapse(&visible_text(&child));
                    if !text.is_empty() {
                        self.out.push(Raw::heading(name.as_bytes()[1] - b'0', text));
                    }
                }
                "pre" => {
                    self.flush();
                    let mut code = String::new();
                    pre_text(&child, &mut code);
                    let code = code.trim_end().trim_start_matches(['\n', '\r']);
                    if !code.trim().is_empty() {
                        self.out.push(Raw::new(BlockKind::Code, code.to_string()));
                    }
                }
                "tr" => {
                    self.flush();
                    let cells: Vec<String> = child
                        .element_children()
                        .into_iter()
                        .filter(|c| matches!(name_of(c).as_str(), "td" | "th") && !hidden(c))
                        .map(|c| collapse(&visible_text(&c)))
                        .skip_while(|c| c.is_empty())
                        .collect();
                    let row = cells.join("\t").trim_end_matches('\t').to_string();
                    if !row.is_empty() {
                        self.out.push(Raw::new(BlockKind::TableRow, row));
                    }
                }
                "li" => {
                    self.flush();
                    self.walk(&child, in_pre, BlockKind::ListItem);
                    self.flush();
                }
                _ if BLOCK.contains(&name.as_str()) => {
                    self.flush();
                    let inner =
                        if kind == BlockKind::ListItem && matches!(name.as_str(), "p" | "div") {
                            kind
                        } else {
                            BlockKind::Paragraph
                        };
                    self.walk(&child, in_pre, inner);
                    self.flush();
                }
                _ => self.walk(&child, in_pre, kind),
            }
        }
    }
}

/// 看得见的文字（跳过 `SKIP` 和隐藏的元素；块级元素和 `br` 处断开）。
fn visible_text(node: &NodeRef) -> String {
    fn go(node: &NodeRef, out: &mut String) {
        for child in node.children() {
            if child.is_text() {
                out.push_str(&child.text());
            } else if child.is_element() {
                let name = name_of(&child);
                if SKIP.contains(&name.as_str()) || hidden(&child) {
                    continue;
                }
                let block = name == "br"
                    || BLOCK.contains(&name.as_str())
                    || matches!(name.as_str(), "td" | "th");
                if block {
                    out.push(' ');
                }
                go(&child, out);
                if block {
                    out.push(' ');
                }
            }
        }
    }
    let mut out = String::new();
    go(node, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_mode_blocks_follow_the_walk() {
        let html = "<html><body><h1>标题</h1><p>一段<br>第二行</p><ul><li>甲<ul><li>乙</li></ul></li></ul><table><tr><td></td><td>a</td><td></td><td>b</td></tr></table><pre>  缩进\n保留</pre><div>散字 <b>加粗</b></div></body></html>";
        let parts = extract(html, None);
        let got: Vec<(&str, &str)> = parts
            .blocks
            .iter()
            .map(|b| (b.kind.key(), b.text.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                ("heading", "标题"),
                ("paragraph", "一段\n第二行"),
                ("list_item", "甲"),
                ("list_item", "乙"),
                ("table_row", "a\t\tb"),
                ("code", "  缩进\n保留"),
                ("paragraph", "散字 加粗")
            ]
        );
        assert_eq!(
            parts.markdown,
            "标题\n\n一段\n第二行\n\n甲\n\n乙\n\na\t\tb\n\n缩进\n保留\n散字 加粗\n"
        );
    }

    #[test]
    fn code_lines_written_as_divs_stay_on_their_own_lines() {
        let html = r#"<html><body><pre><code><div class="cm-line">fn a() {</div><div class="cm-line">  b();</div><div>}</div></code></pre></body></html>"#;
        let parts = extract(html, None);
        assert_eq!(parts.blocks[0].text, "fn a() {\n  b();\n}");
    }
}
