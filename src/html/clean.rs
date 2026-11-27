//! `article` 模式抽正文前的清理：去掉隐藏的、读屏专用的、页内跳转链接、站点地标、空的格式元素；
//! 把 `<pre>` 里的换行（`<br>`、逐行的 `<div>`）写成真的换行，转 Markdown 时代码不挤成一行。
//! 退回整个 `<body>` 时另外去掉页眉、导航（`strip_chrome`）。

use super::{BLOCK, SKIP, hidden, name_of};
use dom_query::{Document, NodeRef};

/// 站点的页眉、导航、页脚、侧栏这类地标（`role`）。
const CHROME_ROLES: &[&str] = &[
    "banner",
    "navigation",
    "contentinfo",
    "complementary",
    "search",
    "menubar",
    "menu",
    "toolbar",
];

/// 抽正文前先整理一遍，交回整理后的 HTML。
pub(super) fn prepare(html: &str) -> String {
    let doc = Document::from(html);
    let drop: Vec<NodeRef> = doc
        .root()
        .descendants()
        .into_iter()
        .filter(|n| {
            n.is_element()
                && (hidden(n)
                    || screen_reader_only(n)
                    || skip_link(n)
                    || chrome_role(n)
                    || empty_inline(n))
        })
        .collect();
    let mut changed = !drop.is_empty();
    for node in drop {
        node.remove_from_parent();
    }
    for pre in doc.select("pre").nodes() {
        changed |= flatten_pre(pre);
    }
    // 只有协议、后面是空的链接（`mailto:`）：Readability 会把它当相对网址拼到网页网址后面，去掉 href 只留文字
    for a in doc.select("a[href]").nodes() {
        if a.attr("href").is_some_and(|h| empty_scheme(&h)) {
            a.remove_attr("href");
            changed = true;
        }
    }
    if changed {
        doc.html().to_string()
    } else {
        html.to_string()
    }
}

/// `<pre>` 里有 `<br>` 或块级元素（逐行一个 `<div>` 的代码高亮）时，换成带换行的纯文字。
/// 外面套的 `<code>` 留着（语言标记在它的类名上）。交回改没改。
fn flatten_pre(pre: &NodeRef) -> bool {
    let structured = pre.descendants().iter().any(|n| {
        n.is_element() && {
            let name = name_of(n);
            name == "br" || BLOCK.contains(&name.as_str())
        }
    });
    if !structured {
        return false;
    }
    let mut text = String::new();
    pre_text(pre, &mut text);
    let text = text.trim_end_matches(['\n', '\r']);
    let elements = pre.element_children();
    let only_code = elements.len() == 1
        && name_of(&elements[0]) == "code"
        && pre
            .children()
            .iter()
            .all(|c| c.is_element() || c.text().trim().is_empty());
    if only_code {
        elements[0].set_text(text);
    } else {
        pre.set_text(text);
    }
    true
}

/// `<pre>` 里的文字原样：`br` 和块级元素处换行，跳过 `SKIP`（复制按钮这类）和隐藏的元素。`full` 模式也用。
pub(super) fn pre_text(node: &NodeRef, out: &mut String) {
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
                continue;
            }
            let block = BLOCK.contains(&name.as_str());
            if block && !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            pre_text(&child, out);
            if block && !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
        }
    }
}

/// `mailto:`、`tel:` 这种只有协议的网址。
fn empty_scheme(href: &str) -> bool {
    href.trim().strip_suffix(':').is_some_and(|scheme| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

/// 空的格式元素（图标字体的 `<i class="icon"></i>` 这类）：转 Markdown 会剩下一对没内容的 `*`。
fn empty_inline(node: &NodeRef) -> bool {
    const INLINE: &[&str] = &[
        "i", "em", "b", "strong", "s", "del", "strike", "u", "mark", "code",
    ];
    INLINE.contains(&name_of(node).as_str())
        && node.text().trim().is_empty()
        && !node.html().contains("<img")
}

fn chrome_role(node: &NodeRef) -> bool {
    node.attr("role")
        .is_some_and(|r| CHROME_ROLES.contains(&r.trim().to_ascii_lowercase().as_str()))
}

/// 读屏专用的元素：看得见的地方通常另有一份，留着会重复。类名常带哈希（`visuallyHidden__TIbnmb`），
/// 所以看去掉 `-`、`_` 后的小写：含 `visuallyhidden`、`screenreader`，或以 `sronly` 开头。
fn screen_reader_only(node: &NodeRef) -> bool {
    node.class().is_some_and(|c| {
        c.split_whitespace().any(|token| {
            let t = token.replace(['-', '_'], "").to_ascii_lowercase();
            t.contains("visuallyhidden") || t.contains("screenreader") || t.starts_with("sronly")
        })
    })
}

/// `<a href="#main">Skip to content</a>`、「跳到正文」。
fn skip_link(node: &NodeRef) -> bool {
    if name_of(node) != "a"
        || !node
            .attr("href")
            .is_some_and(|h| h.trim_start().starts_with('#'))
    {
        return false;
    }
    let text = node.text().trim().to_lowercase();
    [
        "skip to",
        "skip navigation",
        "skip nav",
        "jump to content",
        "跳到",
        "跳转到",
        "跳过导航",
    ]
    .iter()
    .any(|p| text.starts_with(p))
}

/// 去掉站点的页眉、导航这类地标：`role` 是 banner、navigation 等的元素，和不在 `<article>`、`<main>` 里的 `<header>`。
pub(super) fn strip_chrome(doc: &Document) {
    let drop: Vec<NodeRef> = doc
        .root()
        .descendants()
        .into_iter()
        .filter(|n| n.is_element())
        .filter(|n| {
            chrome_role(n)
                || (name_of(n) == "header"
                    && !n
                        .ancestors(None)
                        .iter()
                        .any(|a| matches!(name_of(a).as_str(), "article" | "main")))
        })
        .collect();
    for node in drop {
        node.remove_from_parent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_chrome_is_stripped_for_the_body_fallback() {
        let doc = Document::from(
            r#"<body><header>站点页眉</header><div role="navigation">导航</div><div role="Banner">横幅</div><main><header>文章头</header><p>正文</p></main><article><header>也留</header></article></body>"#,
        );
        strip_chrome(&doc);
        let html = doc.html();
        assert!(
            !html.contains("站点页眉") && !html.contains("导航") && !html.contains("横幅"),
            "{html}"
        );
        assert!(html.contains("文章头") && html.contains("正文") && html.contains("也留"));
    }

    #[test]
    fn hidden_screen_reader_and_skip_links_go_first() {
        let cleaned = prepare(
            r##"<body><a href="#main">Skip to content</a><a href="#top">回到顶部</a><a href="/skip">Skip to the end</a><span class="x sr-only">读屏</span><h2 class="visuallyHidden__TIbnmb">也是读屏</h2><b class="prc-src-InternalVisuallyHidden-2YaI6">还是读屏</b><p>可见</p></body>"##,
        );
        assert!(
            !cleaned.contains("Skip to content") && !cleaned.contains("读屏"),
            "{cleaned}"
        );
        assert!(
            cleaned.contains("回到顶部")
                && cleaned.contains("Skip to the end")
                && cleaned.contains("可见")
        );
        // 类名带哈希的读屏元素也认；类名里只是碰巧有 only 的不算
        assert!(!prepare(r#"<p class="srOnly">读屏</p>"#).contains("读屏"));
        assert!(prepare(r#"<p class="classroom-only">留</p>"#).contains("留"));
        let html = r#"<html><body><p>看得见</p><p hidden>藏起来</p><div aria-hidden="TRUE">也藏</div><span style="Display: None">还藏</span></body></html>"#;
        let cleaned = prepare(html);
        assert!(
            cleaned.contains("看得见") && !cleaned.contains("藏"),
            "{cleaned}"
        );
    }

    #[test]
    fn code_lines_in_pre_become_real_newlines() {
        let text = |html: &str| {
            Document::from(prepare(html).as_str())
                .select("pre")
                .text()
                .to_string()
        };
        // 逐行一个 <div>、行尾 <br>（react.dev 的代码高亮）
        assert_eq!(
            text(
                r#"<pre><code class="language-js"><div class="cm-line">fn a() {<br/></div><div class="cm-line">  b();<br/></div><div>}<br/></div></code></pre>"#
            ),
            "fn a() {\n  b();\n}"
        );
        assert_eq!(
            text("<pre>fn a() {<br>  b();<br>}<button>Copy</button></pre>"),
            "fn a() {\n  b();\n}"
        );
        // 语言标记留着
        assert!(
            prepare(r#"<pre><code class="language-js">a<br>b</code></pre>"#)
                .contains(r#"<code class="language-js">a"#)
        );
        // 本来就是带换行的文字：不动
        let plain =
            "<html><head></head><body><pre><code><span>a</span>\n  b</code></pre></body></html>";
        assert_eq!(prepare(plain), plain);
    }
}
