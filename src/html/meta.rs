//! 标题、作者、日期、站名、语言：Readability 读 meta 和 JSON-LD 的那一套，两种模式共用；标题另修两种情况。

use super::{Parts, name_of};
use crate::{Block, Link};
use dom_query::Document;
use regex::Regex;
use std::sync::OnceLock;

pub(super) struct Meta {
    pub title: String,
    pub byline: Option<String>,
    pub published: Option<String>,
    pub site: Option<String>,
    pub lang: Option<String>,
}

impl Meta {
    /// 只取元数据（`full` 模式、抽正文失败时）。
    pub fn read(html: &str, base: Option<&str>) -> Self {
        match dom_smoothie::Readability::new(html, base, None) {
            Ok(r) => {
                let m = r.get_article_metadata(r.parse_json_ld());
                Self {
                    title: m.title,
                    byline: m.byline,
                    published: m.published_time,
                    site: m.site_name,
                    lang: m.lang,
                }
            }
            Err(_) => Self {
                title: String::new(),
                byline: None,
                published: None,
                site: None,
                lang: None,
            },
        }
    }

    pub fn into_parts(
        self,
        title: Option<String>,
        markdown: String,
        blocks: Vec<Block>,
        links: Vec<Link>,
        needs_js: bool,
    ) -> Parts {
        let some = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        Parts {
            title,
            // 作者里一个字也没有的（人民网 meta 里填的是编号 `103977`）不是作者
            byline: some(self.byline).filter(|b| b.chars().any(char::is_alphabetic)),
            published: some(self.published),
            site: some(self.site),
            lang: some(self.lang),
            markdown,
            blocks,
            links,
            needs_js,
        }
    }
}

/// 标题：Readability 的 → `<title>` → 第一个 `h1`。再修两种情况：
/// - Readability 取的是 `<title>` 里站名那一段（最后一段，或等于 `og:site_name`）：改用第一段
///   （mdBook 的 `<title>What is Ownership? - The Rust Programming Language</title>`，书名在菜单栏的 h1 里）；
/// - 标题是「唯一的 h1 + 分隔符 + 别的」：h1 在正文里（或页面没有 main / article）就用 h1（`配置文件 · Lumen 文档`）；
///   h1 在正文外（菜单栏里的书名）就用别的那段。
pub(super) fn title_of(readability: &str, site: Option<&str>, doc: &Document) -> Option<String> {
    let collapse = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let same = |a: &str, b: &str| collapse(a).to_lowercase() == collapse(b).to_lowercase();
    let raw_title = collapse(&doc.select("title").text());
    let h1s = doc.select("h1");
    let h1 = collapse(&h1s.first().text());
    let mut title = [readability.to_string(), raw_title.clone(), h1.clone()]
        .iter()
        .map(|t| collapse(t))
        .find(|t| !t.is_empty())?;
    let parts = title_parts(&raw_title);
    if parts.len() >= 2
        && (same(&title, parts.last().unwrap()) || site.is_some_and(|s| same(&title, s)))
        && !same(&parts[0], &title)
    {
        title = parts[0].clone();
    }
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
            })
            .map(str::trim)
            .filter(|r| !r.is_empty());
        if let Some(rest) = rest {
            // h1 在正文里（或页面没有正文容器）：它是文章标题；在正文外（菜单栏里的书名、站名）：取剩下那段
            let content = doc.select("main, article, [role=main]");
            let in_content = content.length() == 0
                || h1s.nodes()[0].ancestors(None).iter().any(|a| {
                    matches!(name_of(a).as_str(), "main" | "article")
                        || a.attr("role")
                            .is_some_and(|r| r.eq_ignore_ascii_case("main"))
                });
            return Some(if in_content { h1 } else { rest.to_string() });
        }
    }
    Some(title)
}

/// `<title>` 按常见的分隔符切开（`页名 - 站名`、`页名 | 站名`、`页名_栏目_站名`）。
fn title_parts(title: &str) -> Vec<String> {
    static SEP: OnceLock<Regex> = OnceLock::new();
    let sep = SEP.get_or_init(|| Regex::new(r"\s+[-|–—·»/]\s+|\s+::\s+|\s*[｜_]\s*").unwrap());
    sep.split(title)
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bylines_without_any_letter_are_dropped() {
        let meta = |byline: &str| Meta {
            title: String::new(),
            byline: Some(byline.into()),
            published: None,
            site: None,
            lang: None,
        };
        let parts = |m: Meta| m.into_parts(None, String::new(), Vec::new(), Vec::new(), false);
        assert_eq!(parts(meta(" 103977 ")).byline, None);
        assert_eq!(parts(meta(" 林晓雨 ")).byline.as_deref(), Some("林晓雨"));
        assert_eq!(parts(meta("A. Moss")).byline.as_deref(), Some("A. Moss"));
    }

    #[test]
    fn title_drops_the_site_name_when_it_wraps_the_only_h1() {
        let t = |title: &str, body: &str| title_of(title, None, &Document::from(body));
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
        // Readability 取了站名那段（mdBook：书名在菜单栏的 h1 里，章节标题是 h2）
        let book = r#"<title>What is Ownership? - The Rust Programming Language</title><h1 class="menu-title">The Rust Programming Language</h1><main><h2>What Is Ownership?</h2></main>"#;
        assert_eq!(
            t("The Rust Programming Language", book),
            Some("What is Ownership?".into())
        );
        assert_eq!(
            title_of(
                "示例日报",
                Some("示例日报"),
                &Document::from("<title>地铁通车_城市_示例日报</title>")
            ),
            Some("地铁通车".into())
        );
        assert_eq!(
            t("Home | Survey", "<title>Home | Survey</title>"),
            Some("Home | Survey".into()),
            "整个标题不动"
        );
    }
}
