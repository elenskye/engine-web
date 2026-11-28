//! 认「下一页」（`Article::next_page`，v0.4 Phase 4）：同一篇文章的下一页，不是下一篇。
//!
//! 先看 `<link rel="next">`，再看正文里的链接（`rel="next"`、「下一页」「Next」「次のページ」这类字、页码「2」）。
//! 不管哪来的，网址都要像这一页的下一页：同一个主机，只差一个页号、页号加一
//! （`?page=2`、`/page/2`、`/seite-2`、`news008_2.html`、`/2/`、`index2.html`；第一页可以不带页号）。
//! 网址里别的地方不一样（`/posts/23` → `/posts/24` 这种文章编号之外还要有提示）的不算。

use super::name_of;
use super::sites::text_of;
use dom_query::Document;
use regex::Regex;
use std::sync::OnceLock;
use url::Url;

/// 「下一页」的字（小写，去掉箭头和空白后整个比）。
const NEXT_WORDS: &[&str] = &[
    "next",
    "next page",
    "nextpage",
    "下一页",
    "下页",
    "下一頁",
    "后一页",
    "次へ",
    "次のページ",
    "次のページへ",
    "次ページ",
    "nächste",
    "nächsteseite",
    "weiter",
    "suivant",
    "pagesuivante",
    "siguiente",
    "próxima",
    "successiva",
    "следующая",
    "다음",
    "다음페이지",
];
/// 光秃秃的数字当页号时最大到多少（再大多半是文章编号）。
const MAX_BARE_PAGE: u32 = 50;

pub(super) fn next_page(doc: &Document, base: &Url) -> Option<String> {
    let current = base.as_str().split('#').next().unwrap_or_default();
    let candidate = |href: &str| -> Option<Url> {
        let url = base.join(href.trim()).ok()?;
        let plain = url
            .as_str()
            .split('#')
            .next()
            .unwrap_or_default()
            .to_string();
        (plain != current && is_next(base, &url)).then_some(url)
    };
    for link in doc.select(r#"link[rel~="next"][href]"#).nodes() {
        if let Some(url) = link.attr("href").and_then(|h| candidate(&h)) {
            return Some(clean(url));
        }
    }
    // 正文里的链接：rel=next > 「下一页」这类字 > 页码；同分取前面的
    let here = pages_of(base)
        .into_iter()
        .map(|(_, n)| n)
        .max()
        .unwrap_or(1);
    let mut best: Option<(u8, Url)> = None;
    for a in doc.select("a[href]").nodes() {
        let Some(url) = a.attr("href").and_then(|h| candidate(&h)) else {
            continue;
        };
        let rel = a.attr("rel").unwrap_or_default().to_lowercase();
        let words: String = text_of(a)
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect();
        let score = if rel.split_whitespace().any(|r| r == "next") {
            3
        } else if NEXT_WORDS.iter().any(|w| *w == words)
            || a.attr("class")
                .is_some_and(|c| c.to_lowercase().split_whitespace().any(|c| c == "next"))
            || a.parent().is_some_and(|p| {
                name_of(&p) != "body"
                    && (p.attr("id").as_deref() == Some("next") || p.has_class("next"))
            })
        {
            2
        } else if words == (here + 1).to_string() {
            1
        } else {
            continue;
        };
        if best.as_ref().is_none_or(|(s, _)| score > *s) {
            best = Some((score, url));
        }
    }
    best.map(|(_, u)| clean(u))
}

fn clean(mut url: Url) -> String {
    url.set_fragment(None);
    url.to_string()
}

/// `next` 是 `current` 的下一页：同一个主机、同一个「去掉页号的网址」、页号多一。
pub(crate) fn is_next(current: &Url, next: &Url) -> bool {
    if current.host_str().map(str::to_lowercase) != next.host_str().map(str::to_lowercase)
        || !matches!(next.scheme(), "http" | "https")
    {
        return false;
    }
    let here = pages_of(current);
    let there = pages_of(next);
    here.iter()
        .any(|(stem, n)| there.iter().any(|(s, m)| s == stem && *m == n + 1))
}

/// 一个网址可能的「（去掉页号的网址，第几页）」：不带页号的算第 1 页。
fn pages_of(url: &Url) -> Vec<(String, u32)> {
    static PARAM: OnceLock<Regex> = OnceLock::new();
    static WORD: OnceLock<Regex> = OnceLock::new();
    static SUFFIX: OnceLock<Regex> = OnceLock::new();
    static BARE: OnceLock<Regex> = OnceLock::new();
    static INDEX: OnceLock<Regex> = OnceLock::new();
    let word = WORD.get_or_init(|| {
        Regex::new(r"(?i)^(.*?)/(?:page|seite|pagina|pg|p)[-_/]?(\d{1,4})/?$").unwrap()
    });
    let suffix =
        SUFFIX.get_or_init(|| Regex::new(r"(?i)^(.*/[^/]+?)[_-](\d{1,2})(\.[a-z0-9]+)$").unwrap());
    let bare = BARE.get_or_init(|| Regex::new(r"^(.*?)/(\d{1,2})/?$").unwrap());
    let index =
        INDEX.get_or_init(|| Regex::new(r"(?i)^(.*/)index_?(\d{1,2})\.(s?html?|php)$").unwrap());
    let param = PARAM.get_or_init(|| {
        Regex::new(r"(?i)^(?:page|p|pg|paged|seite|pagina|pagenum|pageno|page_no)$").unwrap()
    });

    let path = url.path();
    let mut pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    pairs.sort();
    let query = |skip: Option<&str>| -> String {
        let kept: Vec<String> = pairs
            .iter()
            .filter(|(k, _)| Some(k.as_str()) != skip)
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        if kept.is_empty() {
            String::new()
        } else {
            format!("?{}", kept.join("&"))
        }
    };
    let stem = |p: &str, q: String| format!("{}{q}", p.trim_end_matches('/'));
    let mut out = vec![(stem(path, query(None)), 1)];
    for (k, v) in &pairs {
        if param.is_match(k) {
            if let Ok(n) = v.parse::<u32>() {
                out.push((stem(path, query(Some(k))), n));
            }
        }
    }
    let q = query(None);
    if let Some(c) = word.captures(path) {
        push(&mut out, stem(&c[1], q.clone()), &c[2], u32::MAX);
    }
    if let Some(c) = suffix.captures(path) {
        push(
            &mut out,
            stem(&format!("{}{}", &c[1], &c[3]), q.clone()),
            &c[2],
            MAX_BARE_PAGE,
        );
    }
    if let Some(c) = bare.captures(path) {
        push(&mut out, stem(&c[1], q.clone()), &c[2], MAX_BARE_PAGE);
    }
    if let Some(c) = index.captures(path) {
        push(
            &mut out,
            stem(&format!("{}index.{}", &c[1], &c[3]), q.clone()),
            &c[2],
            MAX_BARE_PAGE,
        );
    }
    out
}

fn push(out: &mut Vec<(String, u32)>, stem: String, n: &str, max: u32) {
    if let Ok(n) = n.parse::<u32>() {
        if (1..=max).contains(&n) {
            out.push((stem, n));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next(a: &str, b: &str) -> bool {
        is_next(&Url::parse(a).unwrap(), &Url::parse(b).unwrap())
    }

    #[test]
    fn page_numbers_in_urls() {
        let at = "https://atmarkit.itmedia.co.jp/ait/articles/2611/18/news009";
        assert!(next(&format!("{at}.html"), &format!("{at}_2.html")));
        assert!(next(&format!("{at}_2.html"), &format!("{at}_3.html")));
        assert!(
            !next(&format!("{at}_2.html"), &format!("{at}_4.html")),
            "要连着"
        );
        let cb = "https://www.computerbase.de/artikel/gaming/anno-117.100089";
        assert!(next(&format!("{cb}/"), &format!("{cb}/seite-2")));
        assert!(next(&format!("{cb}/seite-2"), &format!("{cb}/seite-3")));
        assert!(next(
            "https://users.rust-lang.org/t/wish-list/137259",
            "https://users.rust-lang.org/t/wish-list/137259?page=2"
        ));
        assert!(next(
            "https://x.test/list?cat=3&page=2",
            "https://x.test/list?page=3&cat=3"
        ));
        assert!(next(
            "https://x.test/review/m1",
            "https://x.test/review/m1/2"
        ));
        assert!(next("https://x.test/blog/", "https://x.test/blog/page/2/"));
        assert!(next(
            "https://x.test/reviews/1/x/index.html",
            "https://x.test/reviews/1/x/index2.html"
        ));
    }

    #[test]
    fn other_articles_are_not_next_pages() {
        // 下一篇：编号、日期、别名不一样
        assert!(!next(
            "https://atmarkit.itmedia.co.jp/ait/articles/2611/18/news008.html",
            "https://atmarkit.itmedia.co.jp/ait/articles/2611/18/news009.html"
        ));
        assert!(!next(
            "https://x.test/posts/123",
            "https://x.test/posts/124"
        ));
        assert!(!next(
            "https://x.test/2026/11/27/a.html",
            "https://x.test/2026/11/28/b.html"
        ));
        assert!(
            !next("https://x.test/a?page=2", "https://y.test/a?page=3"),
            "别的网站"
        );
        assert!(!next("https://x.test/a?page=2", "https://x.test/b?page=3"));
    }

    #[test]
    fn next_links_in_the_page() {
        let base = Url::parse("https://news.example.com/a/story.html").unwrap();
        let find = |html: &str| next_page(&Document::from(html), &base);
        assert_eq!(
            find(r#"<head><link rel="next" href="story_2.html"></head>"#).as_deref(),
            Some("https://news.example.com/a/story_2.html")
        );
        assert_eq!(
            find(r#"<div class="pages"><a href="story_2.html">2</a> <a href="story_3.html">3</a> <a href="story_2.html#top">下一页</a></div>"#).as_deref(),
            Some("https://news.example.com/a/story_2.html")
        );
        // 下一篇（rel=next 但网址不像）、别的链接不算
        assert_eq!(
            find(
                r#"<a rel="next" href="/a/another-story.html">Next post</a><a href="/a/story_9.html">9</a>"#
            ),
            None
        );
        assert_eq!(find("<p>只有一页</p>"), None);
    }
}
