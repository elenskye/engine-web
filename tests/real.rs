//! 真跑（花钱、联网）：都标 `#[ignore]`，验收时手动跑，做法见 `HANDOFF.md`「真跑」。
//!
//! ```text
//! ENGINE_WEB_TAVILY_KEY_FILE=<600 权限的密钥文件> cargo test --release --test real -- --ignored --nocapture
//! ```
//!
//! 本机抓的几条不花钱、不要密钥；Tavily 的几条一共约 6 点（$0.05）。密钥不打印。

use engine_web::engine_core::Cancel;
use engine_web::tavily::{Client, Crawl, Depth, Search, quote};
use engine_web::{FetchOptions, Ureq, extract, fetch};

fn key() -> String {
    let path = std::env::var("ENGINE_WEB_TAVILY_KEY_FILE")
        .expect("要设 ENGINE_WEB_TAVILY_KEY_FILE（600 权限的密钥文件）");
    std::fs::read_to_string(path)
        .expect("读不了密钥文件")
        .trim()
        .to_string()
}

fn credits(what: &str, got: Option<f64>, quoted: f64) {
    println!("{what}：Tavily 报 {got:?} 点，报价 {quoted} 点");
    assert!(got.unwrap_or(0.0) <= quoted, "{what} 实际点数超过了报价");
}

#[test]
#[ignore]
fn real_fetch_follows_redirects_and_reads_pages() {
    let cancel = Cancel::new();
    let opts = FetchOptions::default();
    // MDN 的旧网址会跳到新的（同一个网站）
    let got = fetch(
        "https://developer.mozilla.org/en-US/docs/Web/HTML/Element/table",
        &opts,
        &Ureq,
        &cancel,
    )
    .unwrap();
    assert!(!got.redirects.is_empty(), "{:?}", got.url);
    let a = extract(
        &got.bytes,
        got.content_type.as_deref(),
        &got.url,
        &Default::default(),
    )
    .unwrap();
    assert!(
        a.markdown.contains("<table>"),
        "{}",
        &a.markdown[..200.min(a.markdown.len())]
    );
    // GB18030 的老网站
    let got = fetch("https://www.jjwxc.net/", &opts, &Ureq, &cancel).unwrap();
    let a = extract(
        &got.bytes,
        got.content_type.as_deref(),
        &got.url,
        &Default::default(),
    )
    .unwrap();
    assert_eq!(a.encoding, "gb18030");
    assert!(a.title.as_deref().unwrap_or("").contains("晋江"));
    // 本机地址一个请求都不发
    assert_eq!(
        fetch("http://localhost:3000/", &opts, &Ureq, &cancel).unwrap_err(),
        engine_web::Error::PrivateAddress
    );
}

#[test]
#[ignore]
fn real_tavily_search_extract_crawl_and_map() {
    let key = key();
    let client = Client::new(&key, &Ureq);
    let cancel = Cancel::new();

    let s = Search {
        query: "国家统计局 2026年1—10月份 规模以上工业企业利润".into(),
        max_results: 5,
        ..Default::default()
    };
    let r = client.search(&s, &cancel).unwrap();
    credits("搜索 basic", r.credits, quote::search(Depth::Basic));
    assert!(!r.value.is_empty());
    for h in &r.value {
        println!("  {} | {}", h.title, h.url);
    }

    let urls = vec!["https://www.thepaper.cn/".to_string()];
    let r = client.extract(&urls, Depth::Basic, &cancel).unwrap();
    credits("取页 1 个", r.credits, quote::extract(1, Depth::Basic));
    println!(
        "  取到 {} 页、失败 {} 个；正文 {} 字",
        r.value.pages.len(),
        r.value.failed.len(),
        r.value
            .pages
            .first()
            .map_or(0, |p| p.content.chars().count())
    );

    let c = Crawl {
        url: "https://blog.rust-lang.org/".into(),
        limit: 3,
        ..Default::default()
    };
    let r = client.crawl(&c, &cancel).unwrap();
    credits("爬站 3 页", r.credits, quote::crawl(&c));
    assert!(!r.value.is_empty() && r.value.len() <= 3);
    assert!(
        r.value.iter().all(|p| p.url.contains("rust-lang.org")),
        "不出这个网站"
    );

    let c = Crawl {
        url: "https://doc.rust-lang.org/book/".into(),
        limit: 20,
        ..Default::default()
    };
    let r = client.map(&c, &cancel).unwrap();
    credits("地图 20 个", r.credits, quote::map(20, false));
    assert!(!r.value.is_empty() && r.value.len() <= 20);
    println!("  地图：{} 个，如 {:?}", r.value.len(), r.value.first());
}
