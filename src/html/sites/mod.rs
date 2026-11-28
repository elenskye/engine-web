//! 站点规则（`docs/decisions/0008`）：每站一个模块，交一份 [`Site`]。
//!
//! - 认站只看网址（存好的网页要给原来的网址）。
//! - `fix` 两种模式都做：只改写法、不删正文（把藏起来的正文放出来这类）。
//! - `article` 模式再按 `root` 选正文的根、去掉 `drop`、交 `shape` 整理；选不到根、根里没几个字，就退回通用做法。
//!   没有 `root` 的规则只在整页里去掉 `drop`，正文照通用做法抽。
//! - `build`：正文在网页的 JSON 数据块里（GitHub 的 issue、代码文件），从那里拼出 HTML，拼不出再照 `root`。
//! - `meta`：站点自己写的标题、作者、日期、站名（两种模式都用），盖过通用读到的。

mod aea;
mod arxiv;
mod github;
mod nature;
mod plos;
mod weixin;
mod wikipedia;

use super::name_of;
use dom_query::{Document, NodeRef, Selection};
use url::Url;

pub(crate) struct Site {
    /// 规则的名字，交给 `Article::rule`。
    pub key: &'static str,
    /// 认站（主机、路径）。
    pub matches: fn(&Url) -> bool,
    /// 两种模式都做的改写。
    pub fix: Option<fn(&Document)>,
    /// 正文的根：依次试，用第一个选得到的。空的就照通用做法抽正文。
    pub root: &'static [&'static str],
    /// 去掉的：有 `root` 时在根里找，没有时在整页里找。
    pub drop: &'static [&'static str],
    /// 选好根、去掉 `drop` 以后再整理（只在 `article` 模式）。
    pub shape: Option<fn(&NodeRef)>,
    /// 从网页里的数据块拼正文的 HTML（只在 `article` 模式；交的是没清理过的网页，脚本还在）。
    pub build: Option<fn(&Document) -> Option<String>>,
    /// 站点自己写的标题、作者、日期、站名：有的盖过通用读到的。
    pub meta: Option<fn(&Document, &Url) -> SiteMeta>,
}

/// 什么都不做的规则：各条规则写自己用到的，其余 `..BLANK`。
pub(super) const BLANK: Site = Site {
    key: "",
    matches: |_| false,
    fix: None,
    root: &[],
    drop: &[],
    shape: None,
    build: None,
    meta: None,
};

/// 规则读到的元数据（`Site::meta`）。
#[derive(Default)]
pub(crate) struct SiteMeta {
    pub title: Option<String>,
    pub byline: Option<String>,
    pub published: Option<String>,
    pub site: Option<String>,
}

const SITES: &[&Site] = &[
    &arxiv::SITE,
    &plos::SITE,
    &aea::SITE,
    &nature::SITE,
    &wikipedia::SITE,
    &github::SITE,
    &weixin::SITE,
];

/// 规则选出的正文至少要这么多字（不算空白），不然退回通用做法（网站改版、选择器落空时）。
const MIN_CHARS: usize = 20;

/// 这个网址用哪条规则。
pub(crate) fn detect(base: Option<&Url>) -> Option<&'static Site> {
    detect_in(SITES, base)
}

fn detect_in<'a>(sites: &[&'a Site], base: Option<&Url>) -> Option<&'a Site> {
    let url = base?;
    sites.iter().copied().find(|s| (s.matches)(url))
}

/// 两种模式都做的改写；规则没有 `fix` 时交 `None`（照原样用）。
pub(crate) fn fix(site: &Site, html: &str) -> Option<String> {
    let fix = site.fix?;
    let doc = Document::from(html);
    fix(&doc);
    Some(doc.html().to_string())
}

/// 按规则缩小以后的网页。
pub(crate) enum Narrowed {
    /// 规则选出的正文（HTML 片段），直接转 Markdown。
    Root(String),
    /// 去掉了站点噪声的整页，照通用做法抽正文。
    Page(String),
}

/// `article` 模式：按规则缩小范围。`original` 是没清理过的网页（`build` 用），`html` 是通用清理之后的。
/// 什么都没做成就交 `None`，照通用做法抽原来那份。
pub(crate) fn narrow(site: &Site, original: &str, html: &str) -> Option<Narrowed> {
    if let Some(built) = site
        .build
        .and_then(|build| build(&Document::from(original)))
    {
        // 拼出来的也照通用做法清理一遍（隐藏的、读屏专用的去掉）
        let built = super::clean::prepare(&built);
        if char_count(&Document::from(built.as_str()).root()) >= MIN_CHARS {
            return Some(Narrowed::Root(built));
        }
    }
    let doc = Document::from(html);
    if site.root.is_empty() {
        let mut changed = false;
        for sel in site.drop {
            let found = doc.select(sel);
            changed |= !found.nodes().is_empty();
            found.remove();
        }
        return changed.then(|| Narrowed::Page(doc.html().to_string()));
    }
    let root = site
        .root
        .iter()
        .find_map(|sel| doc.select(sel).nodes().first().copied())?;
    let selection = Selection::from(root);
    for sel in site.drop {
        selection.select(sel).remove();
    }
    if let Some(shape) = site.shape {
        shape(&root);
    }
    (char_count(&root) >= MIN_CHARS).then(|| Narrowed::Root(root.html().to_string()))
}

/// 不算空白的字数。
fn char_count(node: &NodeRef) -> usize {
    node.text().chars().filter(|c| !c.is_whitespace()).count()
}

// —— 各条规则共用的小工具 ——

/// 一个元素里的文字，空白收成一个空格。
pub(super) fn text_of(node: &NodeRef) -> String {
    node.text().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 去掉 `node` 里的链接，只留文字（作者名链到搜索页这类）。
pub(super) fn unlink(node: &NodeRef) {
    node.strip_elements(&["a"]);
}

/// HTML 里的文字转义（规则自己拼 HTML 时用）。
pub(super) fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// 两列的「标签 | 值」表格改成列表：`标签 值`；标签是空的一行接在上一项后面（arXiv 的「Cite as」占三行）。
pub(super) fn label_table_to_list(table: &NodeRef) {
    let mut items: Vec<String> = Vec::new();
    for row in Selection::from(*table).select("tr").nodes() {
        let cells: Vec<NodeRef> = row
            .element_children()
            .into_iter()
            .filter(|c| matches!(name_of(c).as_str(), "td" | "th"))
            .collect();
        let [label, value] = cells.as_slice() else {
            continue;
        };
        unlink_buttons(value);
        let value = value.inner_html().to_string();
        match text_of(label).as_str() {
            "" => {
                if let Some(last) = items.last_mut() {
                    last.push(' ');
                    last.push_str(&value);
                }
            }
            label => items.push(format!("{} {value}", escape(label))),
        }
    }
    let list: String = items.iter().map(|i| format!("<li>{i}</li>")).collect();
    table.replace_with_html(format!("<ul>{list}</ul>"));
}

/// 去掉按钮（「更多说明」这类小按钮里的读屏文字）。
fn unlink_buttons(node: &NodeRef) {
    Selection::from(*node).select("button").remove();
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST: Site = Site {
        key: "test",
        matches: |u| u.host_str() == Some("docs.example.org"),
        fix: Some(|doc| {
            doc.select("#body").remove_attr("style");
        }),
        root: &["#missing", "#body"],
        drop: &[".share"],
        shape: Some(|root| root.strip_elements(&["a"])),
        ..BLANK
    };

    #[test]
    fn detect_looks_at_the_url_only() {
        let url = |u: &str| Url::parse(u).unwrap();
        let sites = [&TEST];
        assert_eq!(
            detect_in(&sites, Some(&url("https://docs.example.org/a"))).map(|s| s.key),
            Some("test")
        );
        assert!(detect_in(&sites, Some(&url("https://example.com/docs.example.org/a"))).is_none());
        assert!(detect_in(&sites, None).is_none(), "没有网址认不出");
        let keys: Vec<&str> = SITES.iter().map(|s| s.key).collect();
        let mut unique = keys.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(keys.len(), unique.len(), "规则名不能重");
    }

    #[test]
    fn root_drop_and_shape_narrow_the_page() {
        let html = r#"<html><body><nav>导航</nav><div id="body" style="display:none"><p>正文第一段，够二十个字了，再多写一些字。</p><div class="share">分享到</div><p><a href="/a">链接文字</a></p></div></body></html>"#;
        let fixed = fix(&TEST, html).unwrap();
        assert!(!fixed.contains("display:none"), "fix 两种模式都做");
        let Some(Narrowed::Root(root)) = narrow(&TEST, &fixed, &fixed) else {
            panic!("应当选到根");
        };
        assert!(root.contains("正文第一段") && root.contains("链接文字"));
        assert!(
            !root.contains("导航") && !root.contains("分享到") && !root.contains("<a "),
            "{root}"
        );
    }

    #[test]
    fn falls_back_when_the_root_is_missing_or_almost_empty() {
        let none = "<html><body><p>别的网页，没有那个根。</p></body></html>";
        assert!(narrow(&TEST, none, none).is_none());
        let empty = r#"<html><body><div id="body"><p>太短</p></div></body></html>"#;
        assert!(narrow(&TEST, empty, empty).is_none(), "根里没几个字");
    }

    #[test]
    fn rules_without_root_only_drop_noise() {
        let site = Site {
            root: &[],
            fix: None,
            shape: None,
            ..TEST
        };
        let html = r#"<html><body><p>正文</p><div class="share">分享到</div></body></html>"#;
        let Some(Narrowed::Page(page)) = narrow(&site, html, html) else {
            panic!("应当去掉噪声");
        };
        assert!(page.contains("正文") && !page.contains("分享到"));
        assert!(narrow(&site, "<p>没有噪声</p>", "<p>没有噪声</p>").is_none());
    }

    #[test]
    fn label_tables_become_lists() {
        let doc = Document::from(
            r#"<table><tr><td>Comments:</td><td>15 pages</td></tr><tr><td>Cite as:</td><td><a href="/x">arXiv:1</a></td></tr><tr><td>&nbsp;</td><td>(or v7)<button><span>Focus</span></button></td></tr></table>"#,
        );
        label_table_to_list(&doc.select("table").nodes()[0]);
        let html = doc.select("ul").html().to_string();
        assert_eq!(
            html,
            r#"<ul><li>Comments: 15 pages</li><li>Cite as: <a href="/x">arXiv:1</a> (or v7)</li></ul>"#
        );
    }
}
