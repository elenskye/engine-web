//! 站点规则（`docs/decisions/0008`）：每站一个模块，交一份 [`Site`]。
//!
//! - 认站只看网址（存好的网页要给原来的网址）。
//! - `fix` 两种模式都做：只改写法、不删正文（把藏起来的正文放出来这类）。
//! - `article` 模式再按 `root` 选正文的根、去掉 `drop`、交 `shape` 整理；选不到根、根里没几个字，就退回通用做法。
//!   没有 `root` 的规则只在整页里去掉 `drop`，正文照通用做法抽。

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
}

const SITES: &[&Site] = &[];

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

/// `article` 模式：按规则缩小范围。`html` 是通用清理之后的网页。什么都没做成就交 `None`，照通用做法抽原来那份。
pub(crate) fn narrow(site: &Site, html: &str) -> Option<Narrowed> {
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
    let chars = root.text().chars().filter(|c| !c.is_whitespace()).count();
    (chars >= MIN_CHARS).then(|| Narrowed::Root(root.html().to_string()))
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
        let Some(Narrowed::Root(root)) = narrow(&TEST, &fixed) else {
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
        assert!(narrow(&TEST, none).is_none());
        let empty = r#"<html><body><div id="body"><p>太短</p></div></body></html>"#;
        assert!(narrow(&TEST, empty).is_none(), "根里没几个字");
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
        let Some(Narrowed::Page(page)) = narrow(&site, html) else {
            panic!("应当去掉噪声");
        };
        assert!(page.contains("正文") && !page.contains("分享到"));
        assert!(narrow(&site, "<p>没有噪声</p>").is_none());
    }
}
