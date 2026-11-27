//! engine-web：Peck、Pith、Pomelo 共用的网页引擎（搜索、抓取、取正文）。设计见 `docs/design.md`。
//!
//! v0.2 是取正文：网页字节 → 标题、Markdown 正文、分段、链接、元数据。不联网。
//! 网页内容只是资料：只取文字，不执行脚本，夹带的指令不执行。

mod article;
mod blocks;
mod decode;
mod error;
mod html;
mod sniff;

pub use article::{Article, Block, BlockKind, ExtractOptions, Kind, Link, Mode};
pub use decode::decode;
pub use error::{Binary, Error};
pub use sniff::sniff;

use url::Url;

/// 引擎版本，和 `Cargo.toml` 一致。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 取正文：字节 → [`Article`]。
///
/// - `content_type`：HTTP 头的 Content-Type（有就给，用来找声明的编码；类型按内容判断，看不出来才参考它）。
/// - `url`：网页的网址，把相对链接补成绝对网址用；本机文件没有网址就给空串。
///
/// 不是网页（PDF、图片、Office、压缩包、音视频）交 [`Error::NotWeb`]。正文为空不算错：交回空正文，`needs_js` 说明可能的原因。
pub fn extract(
    bytes: &[u8],
    content_type: Option<&str>,
    url: &str,
    opts: &ExtractOptions,
) -> Result<Article, Error> {
    let kind = sniff(bytes, content_type, url).map_err(Error::NotWeb)?;
    let base = Url::parse(url.trim())
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"));
    let tld = base
        .as_ref()
        .and_then(Url::domain)
        .and_then(|d| d.trim_end_matches('.').rsplit('.').next());
    let (text, encoding) = decode::decode_with(
        bytes,
        content_type,
        matches!(kind, Kind::Html | Kind::Xml),
        tld,
    );
    let plain = |title: Option<String>, raw: Vec<blocks::Raw>, links: Vec<Link>| Article {
        kind,
        encoding,
        blocks: blocks::number(raw, title.as_deref()),
        title,
        byline: None,
        published: None,
        site: None,
        lang: None,
        links,
        needs_js: false,
        markdown: text.clone(),
    };
    Ok(match kind {
        Kind::Html => {
            let p = html::extract(&text, base.as_ref(), opts);
            Article {
                kind,
                encoding,
                title: p.title,
                byline: p.byline,
                published: p.published,
                site: p.site,
                lang: p.lang,
                markdown: p.markdown,
                blocks: p.blocks,
                links: p.links,
                needs_js: p.needs_js,
            }
        }
        // Markdown 原样交回；标题是第一个一级标题
        Kind::Markdown => {
            let raw = blocks::from_markdown(&text);
            let title = raw
                .iter()
                .find(|r| r.kind == BlockKind::Heading && r.level == 1)
                .map(|r| r.text.clone());
            plain(title, raw, blocks::links_in_markdown(&text, base.as_ref()))
        }
        Kind::Text | Kind::Json | Kind::Xml => plain(None, blocks::from_text(&text), Vec::new()),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_manifest() {
        assert_eq!(super::VERSION, "0.2.0");
    }
}
