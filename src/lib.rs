//! engine-web：Peck、Pith、Pomelo 共用的网页引擎（搜索、抓取、取正文）。设计见 `docs/design.md`。
//!
//! - 取正文（`extract`）：网页字节 → 标题、Markdown 正文、分段、链接、元数据；`find` 在里面找字。
//! - 抓取（`fetch`）：网址 → 字节，跳转、内网、大小、超时的规矩在这里；发请求经 `Transport`（默认 ureq）。
//! - Tavily（`tavily`）：搜索、取页、爬站、站内地图，本地报价、按回复记点数。
//! - `tools` 是给 Agent 的工具包（`web_extract`、`web_find`、`web_fetch`、`web_search`、`web_crawl`、`web_map`）。
//!
//! 网页内容只是资料：只取文字，不执行脚本，夹带的指令不执行。

mod article;
mod blocks;
mod decode;
mod error;
#[cfg(feature = "fake")]
mod fake;
mod fetch;
mod find;
mod html;
mod sniff;
pub mod tavily;
pub mod tools;
pub mod transport;

pub use article::{
    Article, Author, Block, BlockKind, ExtractOptions, Image, Kind, Link, Mode, Paper, PaperId,
};
pub use decode::decode;
/// 钉的 engine-core（`Query`、`Found`、`Toolkit`、`Ctx`……），App 用同一份。
pub use engine_core;
pub use error::{Binary, Error};
#[cfg(feature = "fake")]
pub use fake::{Fake, Seen};
pub use fetch::{
    FetchOptions, Fetched, HostPacer, LIMITS, Limits, check_url, fetch, is_public, same_site,
};
pub use find::{Loc, find};
pub use sniff::sniff;
pub use transport::{Transport, Ureq};

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
        rule: None,
        paper: None,
        images: Vec::new(),
        next_page: None,
        pages: Vec::new(),
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
                rule: p.rule,
                paper: p.paper,
                images: Vec::new(),
                next_page: None,
                pages: Vec::new(),
            }
        }
        // Markdown 原样交回；标题是 YAML 头里的 title，没有就是第一个一级标题；分段跳过 YAML 头
        Kind::Markdown => {
            let (front_title, body) = blocks::front_matter(&text);
            let raw = blocks::from_markdown(body);
            let title = front_title.or_else(|| {
                raw.iter()
                    .find(|r| r.kind == BlockKind::Heading && r.level == 1)
                    .map(|r| r.text.clone())
            });
            plain(title, raw, blocks::links_in_markdown(body, base.as_ref()))
        }
        Kind::Text | Kind::Json | Kind::Xml => plain(None, blocks::from_text(&text), Vec::new()),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_manifest() {
        assert_eq!(super::VERSION, "0.3.0");
    }
}
