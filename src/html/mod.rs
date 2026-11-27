//! HTML → 正文。两种模式：
//!
//! - `article`（默认）：以 Peck 的 `html_to_markdown`、`tidy_markdown` 为底。先清掉隐藏的、读屏专用的、站点地标（`clean`），
//!   Readability 抽正文转 Markdown，太短或像页脚就退回整个 `<body>`，再整理 Markdown（`markdown`）。
//! - `full`：Pith 的 `html_text`（换成 dom_query）走树取全部文字（`full`）。
//!
//! 标题和元数据两种模式共用（`meta`）。和 Peck、Pith 的差别见 `docs/design.md` §4。

mod clean;
mod full;
mod markdown;
mod meta;

use crate::blocks;
use crate::{Block, ExtractOptions, Link, Mode};
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

/// 不读的元素（Pith 的 `SKIP`）：`full` 模式走树、`<pre>` 取字时跳过。
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
/// 正文不到这么多字（不算空白）而有脚本，也提示可能要开 JavaScript。
const JS_EMPTY_CHARS: usize = 20;

pub(crate) fn extract(html: &str, base: Option<&Url>, opts: &ExtractOptions) -> Parts {
    match opts.mode {
        Mode::Article => article(html, base, opts.min_article_chars),
        Mode::Full => full::extract(html, base),
    }
}

fn readability_config() -> dom_smoothie::Config {
    dom_smoothie::Config {
        text_mode: dom_smoothie::TextMode::Markdown,
        ..Default::default()
    }
}

fn article(html: &str, base: Option<&Url>, min_chars: usize) -> Parts {
    let cleaned = clean::prepare(html);
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
    if text.trim().chars().count() < min_chars || looks_like_footer(&text) {
        // 退回整个 <body>：另起一份，去掉站点的页眉、导航这类地标（标题还从原来那份取）
        let doc = Document::from(cleaned.as_str());
        clean::strip_chrome(&doc);
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
    let markdown = markdown::tidy(&text, base);
    let meta = match parsed {
        Some(a) => meta::Meta {
            title: a.title,
            byline: a.byline,
            published: a.published_time,
            site: a.site_name,
            lang: a.lang,
        },
        None => meta::Meta::read(html, base_str),
    };
    let title = meta::title_of(&meta.title, meta.site.as_deref(), &doc);
    let blocks = blocks::number(blocks::from_markdown(&markdown), title.as_deref());
    let needs_js = needs_js(&markdown, html);
    meta.into_parts(title, markdown, blocks, links, needs_js)
}

/// 可能要开 JavaScript：正文不到 500 字而有 5 个以上脚本（Peck 的规则）；
/// 或者正文几乎是空的（不到 20 字）而有脚本（只有一两个 `type="module"` 的单页应用、脚本跳转页）。
/// 数的是原 HTML 里的 `<script>`（不分大小写；`application/ld+json` 这类数据块不算）：清理时删掉的隐藏外层里的也算，
/// 正文藏在 `display:none` 里、靠脚本显示出来的页正要提示（国家自然科学基金委首页）。
fn needs_js(text: &str, html: &str) -> bool {
    static SCRIPT: OnceLock<Regex> = OnceLock::new();
    static TYPE: OnceLock<Regex> = OnceLock::new();
    let script = SCRIPT.get_or_init(|| Regex::new(r"(?i)<script\b([^>]*)>").unwrap());
    let kind = TYPE.get_or_init(|| Regex::new(r#"(?i)\btype\s*=\s*["']?([^"'\s>]*)"#).unwrap());
    let scripts = script
        .captures_iter(html)
        .filter(|c| {
            kind.captures(&c[1]).is_none_or(|t| {
                let t = t[1].to_ascii_lowercase();
                t.is_empty()
                    || t == "module"
                    || t.contains("javascript")
                    || t.contains("ecmascript")
            })
        })
        .count();
    let chars = text.chars().filter(|c| !c.is_whitespace()).count();
    (text.trim().chars().count() < JS_TEXT_CHARS && scripts >= JS_SCRIPTS)
        || (chars < JS_EMPTY_CHARS && scripts >= 1)
}

/// Readability 抽出来的像页脚：不到 1000 字，而且大半的字在带版权或备案字样（`© 2026`、版权所有、ICP 备案……）的段里。
/// 当作没抽到，再看整个 `<body>`。正文末尾一句「本文版权所有」、只写「© 新华社」这种图片来源的都不算。
fn looks_like_footer(text: &str) -> bool {
    static MARK: OnceLock<Regex> = OnceLock::new();
    let mark = MARK.get_or_init(|| {
        Regex::new(r"(?i)©\s*(?:\d{4}|\*)|copyright|all rights reserved|版权所有|ICP\s*备|公网安备")
            .unwrap()
    });
    let count = |s: &str| s.chars().filter(|c| !c.is_whitespace()).count();
    let total = count(text);
    if total == 0 || text.chars().count() >= 1000 {
        return false;
    }
    let marked: usize = text
        .split("\n\n")
        .filter(|p| mark.is_match(p))
        .map(count)
        .sum();
    marked * 2 >= total
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
                    let url = markdown::absolute(href.trim(), base);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://w.example.com/a/b.html").unwrap()
    }

    #[test]
    fn links_are_absolute_http_and_unique() {
        let doc = Document::from(
            r##"<body><a href="/x">一</a><a href="/x">重复</a><a href="mailto:a@b.example">信</a><a href="mailto:">空信</a><a href="javascript:void(0)">脚本</a><a href="#top">锚</a><nav><a href="/nav">导航</a></nav><a hidden href="/h">藏</a></body>"##,
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
    fn footer_means_most_of_the_text_is_copyright_lines() {
        let footer = "版权所有：示例省统计局 联系我们 服务条款\n\n地址：示例市人民路 100 号\n\n示ICP备00000000号-1 示公网安备 00000000000000号 本网站由示例省统计局主办";
        assert!(looks_like_footer(footer));
        assert!(looks_like_footer(
            "Copyright © 2026 Example Inc. All rights reserved."
        ));
        let article = "受连日降温影响，示例市决定将今冬集中供暖启动时间提前五天。市供热办表示，各热源厂已完成检修和燃料储备。\n\n市民如遇家中暖气不热，可拨打供热服务热线反映，供热单位将在 24 小时内上门处理。\n\n（本文版权所有，未经授权不得转载）";
        assert!(!looks_like_footer(article), "正文末尾一句版权声明不算页脚");
        assert!(!looks_like_footer(""));
    }

    #[test]
    fn needs_js_counts_only_runnable_scripts() {
        let page = |head: &str| format!("<html><head>{head}</head><body></body></html>");
        let five = "<SCRIPT src=a.js></SCRIPT>".repeat(5);
        assert!(
            needs_js(
                "很短的正文，但够二十个字了吧，再多写几个字凑数。",
                &page(&five)
            ),
            "大写的也算"
        );
        let data = r#"<script type="application/ld+json">{}</script>"#.repeat(5);
        assert!(
            !needs_js(
                "很短的正文，但够二十个字了吧，再多写几个字凑数。",
                &page(&data)
            ),
            "数据块不算"
        );
        assert!(needs_js(
            "",
            &page(r#"<script type="module" src="app.js"></script>"#)
        ));
        assert!(!needs_js("", &page("")));
    }
}
