//! HTML → 正文。`article` 模式以 Peck 的 `html_to_markdown`、`tidy_markdown` 为底；`full` 模式是 Pith 的 `html_text`（换成 dom_query）。
//!
//! 和 Peck 的差别：隐藏元素（`hidden`、`aria-hidden`、`display:none`、`visibility:hidden`）先去掉再抽正文；
//! 补链接时只把真有协议的网址当绝对网址（Peck 把路径里带 `:` 的也当绝对网址）；不需要的转义还原（`12\.6` → `12.6`）；
//! 标题多两条：`<title>` 是「唯一的 h1 + 分隔符 + 站名」时用 h1，都没有时用第一个 `h1`。

use crate::blocks::{self, Raw};
use crate::{Block, BlockKind, ExtractOptions, Link, Mode};
use dom_query::{Document, NodeRef};
use regex::Regex;
use std::sync::OnceLock;
use url::Url;

/// 取好的 HTML（类型、编码由上层填）。
pub(crate) struct Parts {
    pub title: Option<String>,
    pub byline: Option<String>,
    pub published: Option<String>,
    pub site: Option<String>,
    pub lang: Option<String>,
    pub markdown: String,
    pub blocks: Vec<Block>,
    pub links: Vec<Link>,
    pub needs_js: bool,
}

/// `full` 模式不读的元素（Pith 的 `SKIP`）。
const SKIP: &[&str] = &[
    "script", "style", "noscript", "template", "svg", "math", "head", "iframe", "object", "embed",
    "canvas", "select", "button", "form", "input", "textarea", "audio", "video", "map",
];
/// 块级元素：前后换行（Pith 的 `BLOCK`）。
const BLOCK: &[&str] = &[
    "p",
    "div",
    "section",
    "article",
    "main",
    "header",
    "footer",
    "nav",
    "aside",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "caption",
    "blockquote",
    "pre",
    "figure",
    "figcaption",
    "address",
    "hr",
    "details",
    "summary",
    "center",
];
/// `article` 模式退回整个 `<body>` 时去掉的（Peck 的写法）。
const BODY_SKIP: &[&str] = &[
    "script", "style", "noscript", "nav", "footer", "aside", "svg", "template", "iframe", "form",
];
/// 正文比这短、脚本有这么多，就提示可能要开 JavaScript（Peck 的 `needs_js`）。
const JS_TEXT_CHARS: usize = 500;
const JS_SCRIPTS: usize = 5;

pub(crate) fn extract(html: &str, base: Option<&Url>, opts: &ExtractOptions) -> Parts {
    match opts.mode {
        Mode::Article => article(html, base, opts.min_article_chars),
        Mode::Full => full(html, base),
    }
}

fn readability_config() -> dom_smoothie::Config {
    dom_smoothie::Config {
        text_mode: dom_smoothie::TextMode::Markdown,
        ..Default::default()
    }
}

fn article(html: &str, base: Option<&Url>, min_chars: usize) -> Parts {
    let cleaned = without_hidden(html);
    let base_str = base.map(Url::as_str);
    let parsed =
        dom_smoothie::Readability::new(cleaned.as_str(), base_str, Some(readability_config()))
            .ok()
            .and_then(|mut r| r.parse().ok());
    let doc = Document::from(cleaned.as_str());
    let (mut text, mut links) = match &parsed {
        Some(a) => (a.text_content.to_string(), links_in_html(&a.content, base)),
        None => (String::new(), Vec::new()),
    };
    if text.trim().chars().count() < min_chars {
        let body = doc.select("body");
        let (whole, whole_links) = match body.nodes().first() {
            Some(node) => (
                node.md(Some(BODY_SKIP)).to_string(),
                links_in(node, base, BODY_SKIP),
            ),
            None => (
                doc.md(None).to_string(),
                links_in(&doc.root(), base, BODY_SKIP),
            ),
        };
        if whole.trim().chars().count() > text.trim().chars().count() {
            text = whole;
            links = whole_links;
        }
    }
    let markdown = tidy_markdown(&text, base);
    let meta = match parsed {
        Some(a) => Meta {
            title: a.title,
            byline: a.byline,
            published: a.published_time,
            site: a.site_name,
            lang: a.lang,
        },
        None => metadata(html, base_str),
    };
    let title = title_of(&meta.title, &doc);
    let blocks = blocks::number(blocks::from_markdown(&markdown), title.as_deref());
    let needs_js = needs_js(&markdown, html);
    Parts {
        title,
        byline: some(meta.byline),
        published: some(meta.published),
        site: some(meta.site),
        lang: some(meta.lang),
        markdown,
        blocks,
        links,
        needs_js,
    }
}

fn full(html: &str, base: Option<&Url>) -> Parts {
    let doc = Document::from(html);
    let root = doc.select("html");
    let root = root.nodes().first().cloned().unwrap_or_else(|| doc.root());
    let mut raw = String::new();
    walk(&root, &mut raw, false);
    let markdown = tidy(&raw);
    let mut walker = Blocks::default();
    walker.walk(&root, false, BlockKind::Paragraph);
    walker.flush();
    let meta = metadata(html, base.map(Url::as_str));
    let title = title_of(&meta.title, &doc);
    let blocks = blocks::number(walker.out, title.as_deref());
    let links = links_in(&root, base, SKIP);
    let needs_js = needs_js(&markdown, html);
    Parts {
        title,
        byline: some(meta.byline),
        published: some(meta.published),
        site: some(meta.site),
        lang: some(meta.lang),
        markdown,
        blocks,
        links,
        needs_js,
    }
}

struct Meta {
    title: String,
    byline: Option<String>,
    published: Option<String>,
    site: Option<String>,
    lang: Option<String>,
}

/// 只取元数据（`full` 模式、抽正文失败时）：Readability 读 meta 和 JSON-LD 的那一套。
fn metadata(html: &str, base: Option<&str>) -> Meta {
    match dom_smoothie::Readability::new(html, base, None) {
        Ok(r) => {
            let m = r.get_article_metadata(r.parse_json_ld());
            Meta {
                title: m.title,
                byline: m.byline,
                published: m.published_time,
                site: m.site_name,
                lang: m.lang,
            }
        }
        Err(_) => Meta {
            title: String::new(),
            byline: None,
            published: None,
            site: None,
            lang: None,
        },
    }
}

/// 标题：Readability 的 → `<title>` → 第一个 `h1`。前两个是「唯一的 h1 + 分隔符 + 站名」时用 h1。
fn title_of(readability: &str, doc: &Document) -> Option<String> {
    let collapse = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let h1s = doc.select("h1");
    let h1 = collapse(&h1s.first().text());
    let title = [
        readability.to_string(),
        doc.select("title").text().to_string(),
        h1.clone(),
    ]
    .iter()
    .map(|t| collapse(t))
    .find(|t| !t.is_empty())?;
    if h1s.length() == 1 && !h1.is_empty() && title != h1 {
        const SEPARATORS: &[char] = &[
            '·', '|', '-', '–', '—', '_', ':', '：', '»', '/', '\\', '•', '｜',
        ];
        let rest = title
            .strip_prefix(h1.as_str())
            .map(str::trim_start)
            .and_then(|r| r.strip_prefix(SEPARATORS))
            .or_else(|| {
                title
                    .strip_suffix(h1.as_str())
                    .map(str::trim_end)
                    .and_then(|r| r.strip_suffix(SEPARATORS))
            });
        if rest.is_some_and(|r| !r.trim().is_empty()) {
            return Some(h1);
        }
    }
    Some(title)
}

fn some(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn needs_js(text: &str, html: &str) -> bool {
    text.trim().chars().count() < JS_TEXT_CHARS && html.matches("<script").count() >= JS_SCRIPTS
}

/// 隐藏的元素（Pith 的 `hidden`）：网页里夹带的指令常藏在这里。
fn hidden(node: &NodeRef) -> bool {
    if node.has_attr("hidden")
        || node
            .attr("aria-hidden")
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
    {
        return true;
    }
    let style: String = node
        .attr("style")
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    style.contains("display:none") || style.contains("visibility:hidden")
}

fn name_of(node: &NodeRef) -> String {
    node.node_name()
        .map(|n| n.to_ascii_lowercase())
        .unwrap_or_default()
}

/// 去掉隐藏的元素，交回整理后的 HTML。
fn without_hidden(html: &str) -> String {
    let doc = Document::from(html);
    let hidden_nodes: Vec<NodeRef> = doc
        .root()
        .descendants()
        .into_iter()
        .filter(|n| n.is_element() && hidden(n))
        .collect();
    if hidden_nodes.is_empty() {
        return html.to_string();
    }
    for node in hidden_nodes {
        node.remove_from_parent();
    }
    doc.html().to_string()
}

/// 链接改成绝对网址；图片只留说明（`data:` 的整个去掉）；连着的空行收成一个（Peck 的 `tidy_markdown`）。
fn tidy_markdown(md: &str, base: Option<&Url>) -> String {
    static IMAGE: OnceLock<Regex> = OnceLock::new();
    static LINK: OnceLock<Regex> = OnceLock::new();
    static BLANK: OnceLock<Regex> = OnceLock::new();
    let image =
        IMAGE.get_or_init(|| Regex::new(r#"!\[([^\]]*)\]\(([^)\s]*)(?:\s+"[^"]*")?\)"#).unwrap());
    let link = LINK.get_or_init(|| Regex::new(r#"\]\(([^)\s]+)((?:\s+"[^"]*")?)\)"#).unwrap());
    let blank = BLANK.get_or_init(|| Regex::new(r"\n{3,}").unwrap());
    let text = image.replace_all(md, |c: &regex::Captures| {
        let alt = c[1].trim();
        if alt.is_empty() || c[2].starts_with("data:") {
            String::new()
        } else {
            format!("（图：{alt}）")
        }
    });
    let text = link.replace_all(&text, |c: &regex::Captures| {
        format!("]({}{})", absolute(&c[1], base), &c[2])
    });
    let text = unescape(&text);
    blank.replace_all(text.trim(), "\n\n").to_string()
}

/// dom_query 把 `.`、`(`、`#` 这些都转义了；在那个位置不需要转义的还原，读着顺、和原文对得上。
/// 留着的：会改变 Markdown 含义的（`\*`、`\_`、`` \` ``、`\[`、`\]`、`\<`、`\\`、行首的 `1\.`、`\#`、`\+`、`\>`、表格里的 `\|`、`\![`）。
fn unescape(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut in_fence = false;
    for (i, line) in md.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
        }
        if in_fence || !line.contains('\\') {
            out.push_str(line);
            continue;
        }
        let table = t.starts_with('|');
        let chars: Vec<char> = line.chars().collect();
        let mut j = 0;
        while j < chars.len() {
            let c = chars[j];
            let Some(&next) = chars.get(j + 1).filter(|_| c == '\\') else {
                out.push(c);
                j += 1;
                continue;
            };
            // 前面只有空白和引用记号：算行首
            let at_start = chars[..j].iter().all(|c| c.is_whitespace() || *c == '>');
            let digits_before = chars[..j]
                .iter()
                .rev()
                .take_while(|c| c.is_ascii_digit())
                .count();
            let list_number = digits_before > 0
                && chars[..j - digits_before]
                    .iter()
                    .all(|c| c.is_whitespace() || *c == '>')
                && chars.get(j + 2).is_none_or(|c| c.is_whitespace());
            let keep = match next {
                '.' => list_number,
                '!' => chars.get(j + 2) == Some(&'['),
                '(' | ')' | '{' | '}' => false,
                '#' | '+' | '>' => at_start,
                '|' => table,
                _ => true,
            };
            if keep {
                out.push(c);
            }
            out.push(next);
            j += 2;
        }
    }
    out
}

/// 相对网址补成绝对的；`#…` 和本来就有协议的不动。
fn absolute(target: &str, base: Option<&Url>) -> String {
    if target.starts_with('#') || Url::parse(target).is_ok() {
        return target.to_string();
    }
    base.and_then(|b| b.join(target).ok())
        .map(|u| u.to_string())
        .unwrap_or_else(|| target.to_string())
}

fn links_in_html(fragment: &str, base: Option<&Url>) -> Vec<Link> {
    let doc = Document::from(fragment);
    links_in(&doc.root(), base, BODY_SKIP)
}

/// 一棵树里的 http(s) 链接（跳过 `skip` 里的元素和隐藏的元素），同一网址只留第一次。
fn links_in(node: &NodeRef, base: Option<&Url>, skip: &[&str]) -> Vec<Link> {
    fn go(node: &NodeRef, base: Option<&Url>, skip: &[&str], out: &mut Vec<Link>) {
        for child in node.children() {
            if !child.is_element() {
                continue;
            }
            let name = name_of(&child);
            if skip.contains(&name.as_str()) || hidden(&child) {
                continue;
            }
            if name == "a" {
                if let Some(href) = child.attr("href") {
                    let url = absolute(href.trim(), base);
                    if (url.starts_with("http://") || url.starts_with("https://"))
                        && !out.iter().any(|l| l.url == url)
                    {
                        out.push(Link {
                            text: child
                                .text()
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" "),
                            url,
                        });
                    }
                }
            }
            go(&child, base, skip, out);
        }
    }
    let mut out = Vec::new();
    go(node, base, skip, &mut out);
    out
}

// ---------- full 模式：Pith 的 html_text ----------

/// 走树取全部文字（Pith 的 `walk`）：跳过脚本、样式、表单和隐藏的元素；块级元素前后换行；表格一行一行，格子用制表符隔开。
fn walk(node: &NodeRef, out: &mut String, in_pre: bool) {
    for child in node.children() {
        if child.is_text() {
            let t = child.text();
            if in_pre {
                out.push_str(&t);
            } else {
                // HTML 的空白折叠：连续的空白当一个空格
                let mut last_space = out.ends_with([' ', '\n', '\t']) || out.is_empty();
                for c in t.chars() {
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
                out.push_str(
                    cell.split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .as_str(),
                );
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

/// `full` 模式的分段：和 `walk` 跳过同样的元素，按块级元素切段。
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
            return;
        }
        let mut last_space = self.buf.ends_with([' ', '\n', '\t']) || self.buf.is_empty();
        for c in t.chars() {
            if c.is_whitespace() {
                if !last_space {
                    self.buf.push(' ');
                }
                last_space = true;
            } else {
                self.buf.push(c);
                last_space = false;
            }
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
                    let text = visible_text(&child);
                    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
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
                        .map(|c| {
                            visible_text(&c)
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
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

/// `<pre>` 里的文字原样（`br` 换行，跳过 `SKIP` 和隐藏的元素）。
fn pre_text(node: &NodeRef, out: &mut String) {
    for child in node.children() {
        if child.is_text() {
            out.push_str(&child.text());
        } else if child.is_element() {
            let name = name_of(&child);
            if SKIP.contains(&name.as_str()) || hidden(&child) {
                continue;
            }
            if name == "br" {
                out.push('\n');
            } else {
                pre_text(&child, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://w.example.com/a/b.html").unwrap()
    }

    #[test]
    fn tidy_markdown_makes_links_absolute_and_keeps_only_image_captions() {
        let md = "看[这里](../c/d.html \"标题\")和[锚点](#x)、[外站](https://o.example/p)、[维基](/wiki/Help:Contents)。\n\n\n\n![](a.png)![图注](b.png)![x](data:image/png;base64,AA)";
        assert_eq!(
            tidy_markdown(md, Some(&base())),
            "看[这里](https://w.example.com/c/d.html \"标题\")和[锚点](#x)、[外站](https://o.example/p)、[维基](https://w.example.com/wiki/Help:Contents)。\n\n（图：图注）"
        );
        assert_eq!(
            tidy_markdown("[相对](c.html)", None),
            "[相对](c.html)",
            "没有网址就不补"
        );
    }

    #[test]
    fn needless_escapes_are_undone() {
        assert_eq!(
            unescape(r"全长 12\.6 公里\(约\)，见 \{x\}\! 和 C\# a\>b"),
            "全长 12.6 公里(约)，见 {x}! 和 C# a>b"
        );
        assert_eq!(unescape(r"3\.14 是数"), "3.14 是数");
        assert_eq!(
            unescape(r"1\. 不是列表 \# \+ \* \_ \` \[ \] \< \\"),
            r"1\. 不是列表 # + \* \_ \` \[ \] \< \\"
        );
        assert_eq!(
            unescape("\\# 不是标题\n> \\+ 不是列表\n\\> 不是引用"),
            "\\# 不是标题\n> \\+ 不是列表\n\\> 不是引用"
        );
        assert_eq!(unescape(r"\![不是图](x) | a\|b"), r"\![不是图](x) | a|b");
        assert_eq!(unescape(r"| a\|b | 1\.5 |"), r"| a\|b | 1.5 |");
        assert_eq!(
            unescape("```\n保留 \\. 原样\n```\n12\\.5"),
            "```\n保留 \\. 原样\n```\n12.5"
        );
    }

    #[test]
    fn title_drops_the_site_name_when_it_wraps_the_only_h1() {
        let t = |title: &str, body: &str| title_of(title, &Document::from(body));
        assert_eq!(
            t("配置文件 · Lumen 文档", "<h1>配置文件</h1>"),
            Some("配置文件".into())
        );
        assert_eq!(
            t("Lumen 文档 | 配置文件", "<h1>配置文件</h1>"),
            Some("配置文件".into())
        );
        assert_eq!(
            t("配置文件 · Lumen 文档", "<h1>配置文件</h1><h1>另一个</h1>"),
            Some("配置文件 · Lumen 文档".into()),
            "两个 h1 不猜"
        );
        assert_eq!(
            t("配置文件详解", "<h1>配置文件</h1>"),
            Some("配置文件详解".into()),
            "没有分隔符不动"
        );
        assert_eq!(
            t("", "<title> 示例  &amp; 公报 </title>"),
            Some("示例 & 公报".into())
        );
        assert_eq!(t("", "<h1>只有 h1</h1>"), Some("只有 h1".into()));
        assert_eq!(t("", "<p>都没有</p>"), None);
    }

    #[test]
    fn hidden_elements_are_removed_before_extraction() {
        let html = r#"<html><body><p>看得见</p><p hidden>藏起来</p><div aria-hidden="TRUE">也藏</div><span style="Display: None">还藏</span></body></html>"#;
        let cleaned = without_hidden(html);
        assert!(
            cleaned.contains("看得见") && !cleaned.contains("藏"),
            "{cleaned}"
        );
    }

    #[test]
    fn links_are_absolute_http_and_unique() {
        let doc = Document::from(
            r##"<body><a href="/x">一</a><a href="/x">重复</a><a href="mailto:a@b.example">信</a><a href="javascript:void(0)">脚本</a><a href="#top">锚</a><nav><a href="/nav">导航</a></nav><a hidden href="/h">藏</a></body>"##,
        );
        let links = links_in(&doc.root(), Some(&base()), BODY_SKIP);
        assert_eq!(
            links,
            vec![Link {
                text: "一".into(),
                url: "https://w.example.com/x".into()
            }]
        );
    }

    #[test]
    fn full_mode_blocks_follow_the_walk() {
        let html = "<html><body><h1>标题</h1><p>一段<br>第二行</p><ul><li>甲<ul><li>乙</li></ul></li></ul><table><tr><td></td><td>a</td><td></td><td>b</td></tr></table><pre>  缩进\n保留</pre><div>散字 <b>加粗</b></div></body></html>";
        let parts = full(html, None);
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
}
