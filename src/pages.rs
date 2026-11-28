//! 多页的文章（v0.4 Phase 4，`docs/decisions/0008`）：第一页里认出「下一页」就接着抓，合成一篇。
//!
//! - 只跟同一网站、网址像这一页下一页的（`Article::next_page`）；抓过的网址、和上一页一样的正文就停；每页之前查取消。
//! - 列表页（正文大半是链接的字：首页、目录、搜索结果）不跟：那里的「下一页」是更多条目，不是同一篇。
//! - 中途哪一页没取到（出错、挡程序的验证页、没有正文），交回前面几页，[`Stopped::Failed`] 写明哪一页为什么。
//! - 合成：正文每页之间隔一行「第 N 页：网址」；分段、图都记第几页（段号每页各自从 1 数，和单独抓那一页时一样，能逐页核对）；
//!   链接去重；标题、作者这些用第一页的；`next_page` 是最后一页里的下一页（停在上限时有）。

use crate::{Article, Error, ExtractOptions, FetchOptions, Fetched, Transport, extract, fetch};
use engine_core::Cancel;
use std::collections::HashSet;

/// 默认最多合几页。
pub const DEFAULT_MAX_PAGES: usize = 10;
/// 最多能合几页（工具参数的上限）。
pub const MAX_PAGES: usize = 20;

/// 没接着抓完的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stopped {
    /// 到了最多几页的上限，后面还有（`next`）。
    Limit { next: String },
    /// 这一页没取到：网址和错误；交回的是它前面几页。
    Failed { url: String, error: Error },
}

/// 合好的文章。
#[derive(Debug, Clone)]
pub struct Pages {
    pub article: Article,
    pub stopped: Option<Stopped>,
}

/// 抓到的一篇文章：第一页的抓取结果（原件是第一页的）、合好的正文。
#[derive(Debug, Clone)]
pub struct FetchedArticle {
    pub first: Fetched,
    pub article: Article,
    pub stopped: Option<Stopped>,
}

/// 抓一篇文章：抓第一页、取正文，有下一页就接着抓、合成一篇。`max_pages` 是最多几页（1 就只抓这一页）。
/// 第一页的错照原样交（不是网页交 [`Error::NotWeb`]）；后面哪一页没成写在 `stopped` 里。
pub fn fetch_article(
    url: &str,
    fetch_opts: &FetchOptions,
    transport: &dyn Transport,
    cancel: &Cancel,
    extract_opts: &ExtractOptions,
    max_pages: usize,
) -> Result<FetchedArticle, Error> {
    let first = fetch(url, fetch_opts, transport, cancel)?;
    let article = extract(
        &first.bytes,
        first.content_type.as_deref(),
        &first.url,
        extract_opts,
    )?;
    let pages = follow(article, &first.url, max_pages, cancel, |next| {
        let got = fetch(next, fetch_opts, transport, cancel)?;
        let a = extract(
            &got.bytes,
            got.content_type.as_deref(),
            &got.url,
            extract_opts,
        )?;
        Ok((got.url, a))
    })?;
    Ok(FetchedArticle {
        first,
        article: pages.article,
        stopped: pages.stopped,
    })
}

/// 从取好的第一页（网址 `url`）起接着取：`load` 取一页，交回最后的网址和正文。只有取消算错，别的写进 `stopped`。
pub fn follow(
    first: Article,
    url: &str,
    max_pages: usize,
    cancel: &Cancel,
    mut load: impl FnMut(&str) -> Result<(String, Article), Error>,
) -> Result<Pages, Error> {
    if max_pages <= 1 || first.next_page.is_none() || listing(&first) {
        return Ok(Pages {
            article: first,
            stopped: None,
        });
    }
    let mut seen: HashSet<String> = HashSet::from([plain(url)]);
    let mut pages = vec![(url.to_string(), first)];
    let mut stopped = None;
    while let Some(next) = pages.last().and_then(|(_, a)| a.next_page.clone()) {
        if pages.len() >= max_pages {
            stopped = Some(Stopped::Limit { next });
            break;
        }
        if !seen.insert(plain(&next)) {
            break;
        }
        cancel.check().map_err(|_| Error::Cancelled)?;
        let failed = |error| {
            Some(Stopped::Failed {
                url: next.clone(),
                error,
            })
        };
        match load(&next) {
            Err(Error::Cancelled) => return Err(Error::Cancelled),
            Err(error) => {
                stopped = failed(error);
                break;
            }
            Ok((_, a)) if a.bot_check.is_some() => {
                stopped = failed(Error::BotCheck {
                    by: a.bot_check.unwrap_or("unknown"),
                });
                break;
            }
            Ok((_, a)) if a.markdown.trim().is_empty() => {
                stopped = failed(Error::NoArticle);
                break;
            }
            // 页号超了的网站常回同一页
            Ok((_, a)) if pages.last().is_some_and(|(_, p)| p.markdown == a.markdown) => break,
            Ok((final_url, a)) => {
                seen.insert(plain(&final_url));
                pages.push((final_url, a));
            }
        }
    }
    Ok(Pages {
        article: merge(pages),
        stopped,
    })
}

/// 去掉 `#…`，比网址用。
fn plain(url: &str) -> String {
    url.split('#').next().unwrap_or(url).to_string()
}

/// 列表页：链接的字占了正文（分段的字，不算空白）的一半以上。
fn listing(a: &Article) -> bool {
    let count = |s: &str| s.chars().filter(|c| !c.is_whitespace()).count();
    let total: usize = a.blocks.iter().map(|b| count(&b.text)).sum();
    let linked: usize = a.links.iter().map(|l| count(&l.text)).sum();
    total > 0 && linked * 2 > total
}

/// 几页合成一篇。
fn merge(pages: Vec<(String, Article)>) -> Article {
    let mut pages = pages.into_iter();
    let Some((first_url, mut out)) = pages.next() else {
        unreachable!("至少有第一页");
    };
    let rest: Vec<(String, Article)> = pages.collect();
    if rest.is_empty() {
        return out;
    }
    let mut urls = vec![first_url];
    for b in &mut out.blocks {
        b.page = Some(1);
    }
    for i in &mut out.images {
        i.page = Some(1);
    }
    let mut links: HashSet<String> = out.links.iter().map(|l| l.url.clone()).collect();
    let mut seen_images: HashSet<String> = out.images.iter().map(|i| i.url.clone()).collect();
    for (i, (url, a)) in rest.into_iter().enumerate() {
        let page = i as u32 + 2;
        out.markdown += &format!("\n\n---\n\n第 {page} 页：{url}\n\n");
        out.markdown += &a.markdown;
        out.blocks.extend(a.blocks.into_iter().map(|mut b| {
            b.page = Some(page);
            b
        }));
        // 每页都有的图（栏目图、作者头像）只记第一次
        out.images.extend(
            a.images
                .into_iter()
                .filter(|img| seen_images.insert(img.url.clone()))
                .map(|mut img| {
                    img.page = Some(page);
                    img
                }),
        );
        out.links
            .extend(a.links.into_iter().filter(|l| links.insert(l.url.clone())));
        out.needs_js |= a.needs_js;
        out.next_page = a.next_page;
        urls.push(url);
    }
    out.pages = urls;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExtractOptions, extract};

    fn page(n: u32, next: Option<u32>) -> String {
        let link = next
            .map(|m| format!(r#"<a href="/a/story_{m}.html">下一页</a>"#))
            .unwrap_or_default();
        format!(
            "<html><head><title>故事</title></head><body><article><h1>故事</h1><h2>第 {n} 节</h2><p>{}</p><p><img src=\"/img/{n}.jpg\" width=\"600\" height=\"400\"></p><p>第 {n} 页的结尾。</p>{link}</article></body></html>",
            format!("第 {n} 页的正文，写得够长才像一篇文章的样子。").repeat(8)
        )
    }

    fn url(n: u32) -> String {
        if n == 1 {
            "https://news.example.com/a/story.html".into()
        } else {
            format!("https://news.example.com/a/story_{n}.html")
        }
    }

    fn article(n: u32, next: Option<u32>) -> Article {
        extract(
            page(n, next).as_bytes(),
            None,
            &url(n),
            &ExtractOptions::default(),
        )
        .unwrap()
    }

    #[test]
    fn pages_are_followed_and_merged() {
        let first = article(1, Some(2));
        assert_eq!(first.next_page.as_deref(), Some(url(2).as_str()));
        let mut asked = Vec::new();
        let got = follow(first, &url(1), 10, &Cancel::new(), |u| {
            asked.push(u.to_string());
            let n: u32 = u
                .trim_end_matches(".html")
                .rsplit('_')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            Ok((u.to_string(), article(n, (n < 3).then_some(n + 1))))
        })
        .unwrap();
        assert_eq!(asked, [url(2), url(3)]);
        assert_eq!(got.stopped, None);
        let a = got.article;
        assert_eq!(a.pages, [url(1), url(2), url(3)]);
        assert!(a.markdown.contains(&format!("第 2 页：{}", url(2))));
        assert_eq!(a.next_page, None);
        let second: Vec<_> = a.blocks.iter().filter(|b| b.page == Some(2)).collect();
        assert_eq!(second[0].paragraph, 1, "每页的段号各自从 1 数");
        assert!(
            second
                .iter()
                .any(|b| b.label() == "第 2 页〈第 2 节〉第 2 段")
        );
        assert_eq!(a.images.len(), 3);
        assert_eq!(a.images[2].page, Some(3));
    }

    #[test]
    fn stops_at_the_limit_on_errors_and_on_repeats() {
        let limit = follow(article(1, Some(2)), &url(1), 2, &Cancel::new(), |u| {
            Ok((u.to_string(), article(2, Some(3))))
        })
        .unwrap();
        assert_eq!(limit.article.pages.len(), 2);
        assert_eq!(limit.stopped, Some(Stopped::Limit { next: url(3) }));

        let failed = follow(article(1, Some(2)), &url(1), 10, &Cancel::new(), |_| {
            Err(Error::HttpStatus { status: 500 })
        })
        .unwrap();
        assert!(failed.article.pages.is_empty(), "只有第一页");
        assert_eq!(
            failed.stopped,
            Some(Stopped::Failed {
                url: url(2),
                error: Error::HttpStatus { status: 500 }
            })
        );

        // 第 2 页又指回第 2 页（自己）：抓过的就停
        let looped = follow(article(1, Some(2)), &url(1), 10, &Cancel::new(), |u| {
            let mut a = article(2, None);
            a.next_page = Some(u.to_string());
            Ok((u.to_string(), a))
        })
        .unwrap();
        assert_eq!(looped.article.pages.len(), 2);
        assert_eq!(looped.stopped, None);

        let cancel = Cancel::new();
        cancel.cancel();
        assert_eq!(
            follow(
                article(1, Some(2)),
                &url(1),
                10,
                &cancel,
                |_| unreachable!()
            )
            .unwrap_err(),
            Error::Cancelled
        );
        let one = follow(
            article(1, Some(2)),
            &url(1),
            1,
            &Cancel::new(),
            |_| unreachable!(),
        )
        .unwrap();
        assert_eq!(
            one.article.next_page.as_deref(),
            Some(url(2).as_str()),
            "max_pages 1 只交回下一页的网址"
        );
    }
}
