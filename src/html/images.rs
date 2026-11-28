//! 文中的图（`Article::images`，v0.4 Phase 4）：每张图的网址、替代文字、图注、在第几段后面。
//!
//! - 网址：`srcset`（含 `data-srcset`、`<picture>` 的 `<source>`）取最大的；懒加载的取 `data-src` 这类；
//!   `src` 是占位图（`data:`、`blank.gif` 这类）时不用。补成绝对网址，只留 http(s)。
//! - 不要的：图标（宽高都写了、都不过 48）、1×1 的统计图、表情、头像、徽章（shields.io 这类），重复的只留第一张。
//! - 图注：所在 `<figure>` 的 `<figcaption>`（维基旧写法的 `.thumbcaption`、信息框的 `.infobox-caption`、WordPress 的 `.wp-caption-text` 也认）。
//! - 第几段：图前面那几个字在分段里找（只比字母和数字，不管空白和标点），找到的那一段就是「图前面最近的一段」。
//!   不改正文：Markdown 和分段和没有这一步时一样。

use super::sites::text_of;
use super::{SKIP, name_of};
use crate::{Block, Image};
use dom_query::{NodeRef, Selection};
use url::Url;

/// 懒加载的真网址常放的属性（先到先用）。
const LAZY: &[&str] = &[
    "data-src",
    "data-original",
    "data-lazy-src",
    "data-actualsrc",
    "data-hi-res-src",
    "data-full-src",
    "data-url",
];
/// 网址里有这些的是统计图、徽章（不分大小写）。
const NOT_PICTURES: &[&str] = &[
    "doubleclick.net",
    "google-analytics.com",
    "facebook.com/tr",
    "scorecardresearch.com",
    "pixel.wp.com",
    "stats.wp.com",
    "/b/ss/",
    "spacer.gif",
    "1x1.gif",
    "img.shields.io",
    "badge.svg",
    "badge.fury.io",
    "/badge/",
    "/badges/",
];
/// `class` 里有这些词的不要。
const NOT_PICTURE_CLASSES: &[&str] = &["emoji", "wp-smiley", "avatar", "icon"];
/// 图前面拿来找段的字数（只算字母和数字）；找不到再用短的试。
const CONTEXT: usize = 24;
const SHORT_CONTEXT: usize = 8;

/// 在 `root`（转成正文的那份 HTML）里找图，按 `blocks` 定段号。
pub(super) fn collect(root: &NodeRef, base: Option<&Url>, blocks: &[Block]) -> Vec<Image> {
    let mut walk = Walk {
        base,
        text: String::new(),
        found: Vec::new(),
    };
    walk.go(root);
    let index = Index::new(blocks);
    let mut cursor = 0;
    let mut seen = std::collections::HashSet::new();
    walk.found
        .into_iter()
        .filter(|(image, _)| seen.insert(image.url.clone()))
        .map(|(mut image, before)| {
            image.paragraph = index.locate(&before, &mut cursor);
            image
        })
        .collect()
}

struct Walk<'u> {
    base: Option<&'u Url>,
    /// 走到这里为止的字（只留字母和数字）。
    text: String,
    found: Vec<(Image, String)>,
}

impl Walk<'_> {
    fn go(&mut self, node: &NodeRef) {
        for child in node.children() {
            if child.is_text() {
                self.text
                    .extend(child.text().chars().filter(|c| c.is_alphanumeric()));
            } else if child.is_element() {
                let name = name_of(&child);
                if name == "img" {
                    if let Some(image) = picture(&child, self.base) {
                        let before = tail(&self.text, CONTEXT);
                        self.found.push((image, before));
                    }
                } else if !SKIP.contains(&name.as_str()) {
                    self.go(&child);
                }
            }
        }
    }
}

/// 字符串最后 `n` 个字。
fn tail(s: &str, n: usize) -> String {
    let count = s.chars().count();
    s.chars().skip(count.saturating_sub(n)).collect()
}

/// 一个 `<img>` → 图；不要的交 `None`。
fn picture(img: &NodeRef, base: Option<&Url>) -> Option<Image> {
    let attr = |k: &str| {
        img.attr(k)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    if decoration(img) {
        return None;
    }
    let srcset = attr("srcset")
        .or_else(|| attr("data-srcset"))
        .or_else(|| picture_source(img));
    let raw = srcset
        .as_deref()
        .and_then(largest)
        .or_else(|| LAZY.iter().find_map(|k| attr(k)))
        .or_else(|| attr("src").filter(|s| !placeholder(s)))?;
    let url = absolute(&raw, base)?;
    let canonical = attr("data-canonical-src").unwrap_or_default();
    let lower = format!("{} {}", url.to_lowercase(), canonical.to_lowercase());
    if NOT_PICTURES.iter().any(|p| lower.contains(p)) {
        return None;
    }
    Some(Image {
        url,
        alt: attr("alt").map(|a| a.split_whitespace().collect::<Vec<_>>().join(" ")),
        caption: caption(img),
        paragraph: None,
        page: None,
    })
}

/// 图标、统计用的小图、表情、头像。
fn decoration(img: &NodeRef) -> bool {
    let size = |k: &str| {
        img.attr(k).and_then(|v| {
            v.trim()
                .trim_end_matches("px")
                .parse::<f32>()
                .ok()
                .map(|n| n as u32)
        })
    };
    let (w, h) = (size("width"), size("height"));
    let tiny = matches!((w, h), (Some(w), Some(h)) if w <= 48 && h <= 48);
    let pixel = w.is_some_and(|w| w <= 2) || h.is_some_and(|h| h <= 2);
    let class = img.attr("class").unwrap_or_default().to_lowercase();
    let decorative_class = class
        .split_whitespace()
        .any(|c| NOT_PICTURE_CLASSES.contains(&c));
    tiny || pixel || decorative_class
}

/// `<picture>` 里第一个 `<source srcset>`。
fn picture_source(img: &NodeRef) -> Option<String> {
    let parent = img.parent()?;
    if name_of(&parent) != "picture" {
        return None;
    }
    let sources = Selection::from(parent).select("source");
    sources
        .nodes()
        .iter()
        .find_map(|s| s.attr("srcset").or_else(|| s.attr("data-srcset")))
        .map(|s| s.to_string())
}

/// `srcset` 里最大的一张（按 `500w` 的宽或 `2x` 的倍数；都没写的算 1x）。
pub(super) fn largest(srcset: &str) -> Option<String> {
    let mut best: Option<(f32, String)> = None;
    let mut rest = srcset.trim();
    while !rest.is_empty() {
        rest = rest.trim_start_matches(|c: char| c == ',' || c.is_whitespace());
        if rest.is_empty() {
            break;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let mut url = &rest[..end];
        rest = &rest[end..];
        let descriptor = if url.ends_with(',') {
            url = url.trim_end_matches(',');
            ""
        } else {
            let stop = rest.find(',').unwrap_or(rest.len());
            let d = rest[..stop].trim();
            rest = &rest[stop..];
            d
        };
        let score = descriptor
            .strip_suffix('w')
            .and_then(|n| n.trim().parse::<f32>().ok())
            .or_else(|| {
                descriptor
                    .strip_suffix('x')
                    .and_then(|n| n.trim().parse::<f32>().ok())
            })
            .unwrap_or(1.0);
        if !url.is_empty() && best.as_ref().is_none_or(|(s, _)| score > *s) {
            best = Some((score, url.to_string()));
        }
    }
    best.map(|(_, u)| u)
}

/// 懒加载时 `src` 里放的占位图。
fn placeholder(src: &str) -> bool {
    let s = src.to_lowercase();
    let file = s.rsplit('/').next().unwrap_or(&s);
    s.starts_with("data:")
        || [
            "blank.",
            "spacer.",
            "pixel.",
            "transparent.",
            "placeholder",
            "lazy",
            "loading.",
        ]
        .iter()
        .any(|p| file.starts_with(p))
}

fn absolute(raw: &str, base: Option<&Url>) -> Option<String> {
    let url = match base {
        Some(b) => b.join(raw).ok()?,
        None => Url::parse(raw).ok()?,
    };
    matches!(url.scheme(), "http" | "https").then(|| url.to_string())
}

/// 图注：所在 `<figure>` 的 `<figcaption>`，或维基旧写法、WordPress 的图注。
fn caption(img: &NodeRef) -> Option<String> {
    for a in img.ancestors(Some(6)) {
        let selector = match name_of(&a).as_str() {
            "figure" => "figcaption",
            _ if a.has_class("thumb") || a.has_class("thumbinner") => ".thumbcaption",
            _ if a.has_class("wp-caption") => ".wp-caption-text",
            _ if a.has_class("infobox-image") => ".infobox-caption",
            _ => continue,
        };
        let found = Selection::from(a).select(selector);
        if let Some(c) = found.nodes().first() {
            let text = text_of(c);
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

/// 分段的字（只留字母和数字）连成一串，记下每段从哪里起。
struct Index {
    all: String,
    starts: Vec<usize>,
    paragraphs: Vec<u32>,
}

impl Index {
    fn new(blocks: &[Block]) -> Self {
        let mut all = String::new();
        let mut starts = Vec::with_capacity(blocks.len());
        let mut paragraphs = Vec::with_capacity(blocks.len());
        for b in blocks {
            starts.push(all.len());
            paragraphs.push(b.paragraph);
            all.extend(b.text.chars().filter(|c| c.is_alphanumeric()));
        }
        Self {
            all,
            starts,
            paragraphs,
        }
    }

    /// `before`（图前面的字）落在哪一段；从 `cursor` 往后找，找到了把 `cursor` 挪过去（图按顺序）。
    fn locate(&self, before: &str, cursor: &mut usize) -> Option<u32> {
        if before.is_empty() {
            return None;
        }
        for n in [CONTEXT, SHORT_CONTEXT] {
            let needle = tail(before, n);
            let from = (*cursor).min(self.all.len());
            let hit = self.all[from..]
                .find(&needle)
                .map(|p| from + p)
                .or_else(|| self.all.find(&needle));
            if let Some(pos) = hit {
                let end = pos + needle.len();
                *cursor = end;
                let i = self.starts.partition_point(|&s| s < end).saturating_sub(1);
                return self.paragraphs.get(i).copied();
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dom_query::Document;

    #[test]
    fn srcset_picks_the_largest() {
        assert_eq!(
            largest("a.jpg 320w, b.jpg 1024w, c.jpg 640w").as_deref(),
            Some("b.jpg")
        );
        assert_eq!(largest("a.jpg, b.jpg 2x").as_deref(), Some("b.jpg"));
        assert_eq!(
            largest("https://x.test/w_100,h_100/a.jpg 1x, https://x.test/w_200,h_200/a.jpg 2x")
                .as_deref(),
            Some("https://x.test/w_200,h_200/a.jpg")
        );
        assert_eq!(largest("  "), None);
    }

    #[test]
    fn lazy_icons_pixels_and_badges() {
        let base = Url::parse("https://news.example.com/a/b.html").unwrap();
        let doc = Document::from(
            r#"<body><p>第一段文字。</p>
            <figure><img src="data:image/gif;base64,R0l" data-src="/img/1.jpg" alt=" 一张 图 "><figcaption>图 1：河流</figcaption></figure>
            <p>第二段文字。<img src="/icons/rss.png" width="16" height="16"><img src="https://pixel.wp.com/g.gif?x=1"><img src="/t.gif" width="1" height="1"></p>
            <p><img src="https://img.shields.io/badge/ci-passing-green.svg" alt="ci"><img src="/img/1.jpg"></p>
            <p>第三段。<img srcset="/s.jpg 400w, /l.jpg 1200w" src="/s.jpg"></p></body>"#,
        );
        let blocks = crate::blocks::number(
            crate::blocks::from_markdown("第一段文字。\n\n图 1：河流\n\n第二段文字。\n\n第三段。"),
            None,
        );
        let root = doc.select("body").nodes()[0];
        let images = collect(&root, Some(&base), &blocks);
        let urls: Vec<&str> = images.iter().map(|i| i.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://news.example.com/img/1.jpg",
                "https://news.example.com/l.jpg"
            ]
        );
        assert_eq!(images[0].alt.as_deref(), Some("一张 图"));
        assert_eq!(images[0].caption.as_deref(), Some("图 1：河流"));
        assert_eq!(images[0].paragraph, Some(1), "图前面最近的一段");
        assert_eq!(images[1].paragraph, Some(4));
    }

    #[test]
    fn an_image_before_any_text_has_no_paragraph() {
        let doc = Document::from(r#"<body><img src="https://x.test/top.jpg"><p>正文。</p></body>"#);
        let blocks = crate::blocks::number(crate::blocks::from_markdown("正文。"), None);
        let images = collect(&doc.select("body").nodes()[0], None, &blocks);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].paragraph, None);
    }
}
