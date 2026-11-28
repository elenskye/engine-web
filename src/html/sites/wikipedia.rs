//! 维基百科的条目（`*.wikipedia.org/wiki/…`；手机版 `*.m.wikipedia.org`、中文的 `/zh-cn/…` 这类、`/w/index.php` 的旧版本也算）。
//! 正文是 `#mw-content-text` 里的 `.mw-parser-output`（新的 Parsoid 写法和旧写法都是）。
//!
//! - 信息框读成两列的表：框名单独一行在前，表里的图挪到表前面（`images` 里有、正文只留说明）。
//! - 引用角标留成「[1]」文字（不带链接）；参考文献每条前面写同样的「[1]」，和正文对得上（注释组的「[a]」也一样）。
//! - 去掉「编辑」链接、红链（还没写的条目）的编辑网址、导航框、侧栏、「其他用法」提示、维护模板、目录、坐标、
//!   参考文献前面的「^ a b」回链。
//! - 公式：图片的替代文字是 TeX，写成 `$…$`（行内）、`$$…$$`（单独一行），放进代码里免得被转义。
//! - 表格里正文行的 `<th>`（行头）当普通格子，不然转出来行头全挤进表头。

use super::{BLANK, Site, escape, text_of};
use crate::html::name_of;
use dom_query::{NodeRef, Selection};
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;
use url::Url;

pub(super) const SITE: Site = Site {
    key: "wikipedia",
    matches,
    root: &["#mw-content-text > .mw-parser-output", "#mw-content-text"],
    drop: &[
        // 「编辑」「编辑源代码」
        ".mw-editsection",
        // 导航框、「Part of a series on」侧栏、规范控制、姊妹项目的框、主题入口
        ".navbox",
        ".navbox-styles",
        ".vertical-navbox",
        ".sidebar",
        ".authority-control",
        ".sistersitebox",
        ".side-box",
        ".portalbox",
        ".portal-bar",
        // 「其他用法」「重定向到这里」这类提示、维护模板、短说明
        ".hatnote",
        ".dablink",
        ".rellink",
        ".ambox",
        ".ombox",
        ".tmbox",
        ".cmbox",
        ".mbox-small",
        ".shortdescription",
        // 目录、右上角的坐标、参考文献的回链、图上的放大按钮、空段
        "#toc",
        ".toc",
        "#coordinates",
        ".mw-cite-backlink",
        ".magnify",
        ".mw-empty-elt",
        ".catlinks",
        ".printfooter",
        "style",
        "link",
    ],
    shape: Some(shape),
    ..BLANK
};

/// 中文维基的语言变体路径（`/zh-cn/条目`）。
const ZH_VARIANTS: &[&str] = &[
    "/zh/",
    "/zh-cn/",
    "/zh-tw/",
    "/zh-hk/",
    "/zh-sg/",
    "/zh-mo/",
    "/zh-my/",
    "/zh-hans/",
    "/zh-hant/",
];

fn matches(u: &Url) -> bool {
    let wikipedia = u.host_str().is_some_and(|h| h.ends_with(".wikipedia.org"));
    let path = u.path();
    wikipedia
        && (path.starts_with("/wiki/")
            || path == "/w/index.php"
            || ZH_VARIANTS.iter().any(|v| path.starts_with(v)))
}

fn shape(root: &NodeRef) {
    let sel = Selection::from(*root);
    formulas(&sel);
    let labels = citations(&sel);
    references(&sel, &labels);
    // 红链：链到「编辑这个还没写的条目」，只留文字
    for a in sel.select("a.new").nodes() {
        a.replace_with_html(a.inner_html());
    }
    for table in sel.select("table.infobox").nodes() {
        // 信息框里套的小信息框跟着外面那个一起读
        let nested = table
            .ancestors(None)
            .iter()
            .any(|a| name_of(a) == "table" && a.has_class("infobox"));
        if !nested {
            infobox(table);
        }
    }
    for table in sel.select("table").nodes() {
        row_headers_as_cells(table);
    }
    // 图的说明单成一段（不然相邻两张图的说明连成一句）
    for caption in sel.select("figcaption").nodes() {
        caption.set_html(format!("<p>{}</p>", caption.inner_html()));
    }
}

/// 公式 → `$TeX$`；单独一行的 → `$$TeX$$`。TeX 从 Parsoid 的 `data-mw`（`body.extsrc`）、MathML 的 `alttext`、
/// 旧写法的图片替代文字里取。
fn formulas(sel: &Selection) {
    static WRAP: OnceLock<Regex> = OnceLock::new();
    let wrap = WRAP.get_or_init(|| {
        Regex::new(r"^\{\\(?:displaystyle|textstyle|scriptstyle)\s*(.*)\}$").unwrap()
    });
    for el in sel.select(".mwe-math-element").nodes() {
        let math = Selection::from(*el);
        let source = el
            .attr("data-mw")
            .and_then(|d| serde_json::from_str::<serde_json::Value>(&d).ok())
            .and_then(|d| d["body"]["extsrc"].as_str().map(str::to_string));
        let tex = source
            .or_else(|| el.attr("alttext").map(|t| t.to_string()))
            .or_else(|| math.select("math").attr("alttext").map(|t| t.to_string()))
            .or_else(|| math.select("img").attr("alt").map(|t| t.to_string()))
            .map(|t| t.trim().to_string())
            .unwrap_or_default();
        if tex.is_empty() {
            continue;
        }
        let tex = wrap
            .captures(&tex)
            .map(|c| c[1].trim().to_string())
            .unwrap_or(tex);
        let block = el.has_class("mwe-math-element-block")
            || el.attr("display").is_some_and(|d| &*d == "block")
            || math.select(".mwe-math-fallback-image-display").exists();
        let in_paragraph = el.parent().is_some_and(|p| name_of(&p) == "p");
        el.replace_with_html(match (block, in_paragraph) {
            (true, false) => format!("<p><code>$${}$$</code></p>", escape(&tex)),
            (true, true) => format!("<code>$${}$$</code>", escape(&tex)),
            _ => format!("<code>${}$</code>", escape(&tex)),
        });
    }
}

/// 引用角标换成文字（「[1]」「[a]」「[note 1]」），交回「参考文献条目的 id → 角标」。
fn citations(sel: &Selection) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    for sup in sel.select("sup.reference").nodes() {
        let text = text_of(sup);
        if text.is_empty() {
            continue;
        }
        let label = if text.starts_with('[') {
            text
        } else {
            format!("[{text}]")
        };
        let target = Selection::from(*sup)
            .select("a")
            .attr("href")
            .and_then(|h| h.strip_prefix('#').map(str::to_string));
        if let Some(id) = target {
            labels.entry(id).or_insert_with(|| label.clone());
        }
        sup.replace_with_html(escape(&label));
    }
    labels
}

/// 参考文献（`<ol class="references">`）：每条前面写正文里的角标，改成不带序号的列表（注释组用字母，序号会对不上）。
fn references(sel: &Selection, labels: &HashMap<String, String>) {
    for ol in sel.select("ol.references").nodes() {
        let mut items = String::new();
        let lis = ol
            .element_children()
            .into_iter()
            .filter(|c| name_of(c) == "li");
        for (i, li) in lis.enumerate() {
            let label = li
                .attr("id")
                .and_then(|id| labels.get(id.as_ref()).cloned())
                .unwrap_or_else(|| format!("[{}]", i + 1));
            let text = Selection::from(li).select(".reference-text");
            let body = match text.nodes().first() {
                Some(t) => t.inner_html(),
                None => li.inner_html(),
            };
            items += &format!("<li>{} {body}</li>", escape(&label));
        }
        ol.replace_with_html(format!("<ul>{items}</ul>"));
    }
}

/// 表的行（`<thead>`、`<tbody>` 里的也算，套着的小表不算）。
fn rows<'a>(table: &NodeRef<'a>) -> Vec<NodeRef<'a>> {
    let mut out = Vec::new();
    for child in table.element_children() {
        match name_of(&child).as_str() {
            "tr" => out.push(child),
            "thead" | "tbody" | "tfoot" => out.extend(
                child
                    .element_children()
                    .into_iter()
                    .filter(|c| name_of(c) == "tr"),
            ),
            _ => {}
        }
    }
    out
}

fn cells<'a>(row: &NodeRef<'a>) -> Vec<NodeRef<'a>> {
    row.element_children()
        .into_iter()
        .filter(|c| matches!(name_of(c).as_str(), "th" | "td"))
        .collect()
}

/// 信息框 → 框名（加粗的一段）+ 表前的图 + 两列的表。
/// 第一行当表头：Markdown 的表要有表头，表头里空的格子转出来不成表，所以开头只有一格的行（别的语言的名字这类）也挪到表前。
fn infobox(table: &NodeRef) {
    // 旗子、「维基共享资源」这类小图标不要
    for img in Selection::from(*table).select("img").nodes() {
        if icon(img) {
            img.remove_from_parent();
        }
    }
    // 框名、开头的小标题：HTML（多语言的名字用 <br> 隔开，收成一行）
    let mut heads: Vec<String> = Selection::from(*table)
        .select("caption")
        .nodes()
        .first()
        .map(inline)
        .filter(|t| !t.is_empty())
        .into_iter()
        .collect();
    let mut figures = String::new();
    let mut pairs: Vec<(String, String)> = Vec::new();
    for row in rows(table) {
        let cells = cells(&row);
        match cells.as_slice() {
            [] => {}
            [one] => {
                let class = one.attr("class").unwrap_or_default().to_string();
                let cell = Selection::from(*one);
                if class.contains("infobox-image") || cell.select("img").exists() {
                    figures += &figure(&cell);
                    continue;
                }
                let html = inline(one);
                if text_of(one).is_empty() {
                    continue;
                }
                let heading = name_of(one) == "th" || class.contains("header");
                if pairs.is_empty() {
                    heads.push(html);
                } else if heading {
                    pairs.push((format!("<b>{html}</b>"), String::new()));
                } else {
                    pairs.push((html, String::new()));
                }
            }
            [label, values @ ..] => {
                let values: Vec<String> = values.iter().map(inline).collect();
                pairs.push((inline(label), values.join(" · ")));
            }
        }
    }
    if pairs.is_empty() {
        return;
    }
    heads.dedup();
    let mut out: String = heads.iter().map(|h| format!("<p><b>{h}</b></p>")).collect();
    out += &figures;
    out += "<table>";
    for (i, (label, value)) in pairs.iter().enumerate() {
        let tag = if i == 0 { "th" } else { "td" };
        out += &format!("<tr><{tag}>{label}</{tag}><{tag}>{value}</{tag}></tr>");
    }
    out += "</table>";
    table.replace_with_html(out);
}

/// 小图标：宽、高都写了、都不过 50。
fn icon(img: &NodeRef) -> bool {
    let size = |k: &str| img.attr(k).and_then(|v| v.trim().parse::<u32>().ok());
    matches!((size("width"), size("height")), (Some(w), Some(h)) if w <= 50 && h <= 50)
}

/// 信息框里的图：图和说明挪成 `<figure>`（说明是 `.infobox-caption` 或格子里别的字）。
fn figure(cell: &Selection) -> String {
    let caption = cell.select(".infobox-caption");
    let caption = match caption.nodes().first() {
        Some(c) => text_of(c),
        None => cell.nodes().first().map(text_of).unwrap_or_default(),
    };
    let images: String = cell
        .select("img")
        .nodes()
        .iter()
        .map(|img| {
            let attr = |k: &str| {
                img.attr(k)
                    .map(|v| format!(r#" {k}="{}""#, escape(&v).replace('"', "&quot;")))
                    .unwrap_or_default()
            };
            format!(
                "<img{}{}{}{}{}>",
                attr("src"),
                attr("srcset"),
                attr("alt"),
                attr("width"),
                attr("height")
            )
        })
        .collect();
    if caption.is_empty() {
        format!("<figure>{images}</figure>")
    } else {
        format!(
            "<figure>{images}<figcaption>{}</figcaption></figure>",
            escape(&caption)
        )
    }
}

/// 格子里的内容收成一行：换行、列表项、段之间用「; 」隔开，块级的标签去掉（链接、粗体留着）。
fn inline(cell: &NodeRef) -> String {
    static BREAK: OnceLock<Regex> = OnceLock::new();
    static BLOCK: OnceLock<Regex> = OnceLock::new();
    static SPACE: OnceLock<Regex> = OnceLock::new();
    let brk = BREAK.get_or_init(|| {
        Regex::new(r"(?i)<br\s*/?>|</(?:li|p|div|tr|dd)>\s*<(?:li|p|div|tr|dt|dd)\b[^>]*>").unwrap()
    });
    let block = BLOCK.get_or_init(|| {
        Regex::new(r"(?i)</?(?:ul|ol|li|p|div|table|tbody|thead|tfoot|tr|td|th|dl|dt|dd|figure|figcaption|center)\b[^>]*>").unwrap()
    });
    let space = SPACE.get_or_init(|| Regex::new(r"\s+").unwrap());
    let html = cell.inner_html();
    let html = brk.replace_all(&html, "; ");
    let html = block.replace_all(&html, " ");
    let html = space.replace_all(&html, " ");
    html.trim().trim_matches(';').trim().to_string()
}

/// 正文行里的 `<th>`（行头）改成 `<td>`：第一行是表头不动。
fn row_headers_as_cells(table: &NodeRef) {
    for row in rows(table).iter().skip(1) {
        for cell in cells(row) {
            if name_of(&cell) == "th" {
                cell.rename("td");
            }
        }
    }
}
