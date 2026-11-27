//! Tavily 客户端（模拟服务，不花钱）：请求体、点数、出错、重试、取消。回复的样子照官方文档（2026-11-27）。
#![cfg(feature = "fake")]

use engine_web::Error;
use engine_web::Fake;
use engine_web::engine_core::Cancel;
use engine_web::tavily::{Client, Crawl, Depth, Search};
use engine_web::transport::{Method, Response};
use serde_json::json;
use std::time::{Duration, Instant};

const SEARCH: &str = r#"{
  "query": "示例市 青年失业率",
  "results": [
    {"title": "示例市统计公报", "url": "https://stats.example.org/bulletin", "content": "2026 年青年失业率为……", "score": 0.91, "published_date": "2026-02-20", "raw_content": null},
    {"title": "", "url": "https://survey.example.org/youth", "content": "调查显示……", "score": 0.5},
    {"title": "没有网址的不要", "content": "x"}
  ],
  "response_time": 1.2,
  "usage": {"credits": 1},
  "request_id": "123e4567-e89b-12d3-a456-426614174111"
}"#;

fn json_reply(status: u16, body: &str) -> Response {
    Response::with(status, "application/json", body)
}

fn search_url() -> &'static str {
    "https://api.tavily.com/search"
}

#[test]
fn search_sends_the_documented_body_and_reads_credits() {
    let fake = Fake::new().reply(Method::Post, search_url(), json_reply(200, SEARCH));
    let client = Client::new("tvly-dev-fake", &fake);
    let s = Search {
        query: " 示例市 青年失业率 ".into(),
        max_results: 50,
        depth: Depth::Advanced,
        topic: "news".into(),
        recency: Some("month".into()),
        domains: vec!["stats.example.org".into()],
        exclude_domains: vec![],
    };
    let reply = client.search(&s, &Cancel::new()).unwrap();
    assert_eq!(reply.credits, Some(1.0));
    assert_eq!(
        reply.request_id.as_deref(),
        Some("123e4567-e89b-12d3-a456-426614174111")
    );
    assert_eq!(
        reply
            .value
            .iter()
            .map(|h| h.url.as_str())
            .collect::<Vec<_>>(),
        [
            "https://stats.example.org/bulletin",
            "https://survey.example.org/youth"
        ]
    );
    assert_eq!(reply.value[0].published.as_deref(), Some("2026-02-20"));
    let seen = &fake.requests()[0];
    assert_eq!(seen.header("authorization"), Some("Bearer tvly-dev-fake"));
    assert_eq!(seen.header("content-type"), Some("application/json"));
    assert!(seen.pinned.is_empty());
    assert_eq!(
        seen.json(),
        json!({
            "query": "示例市 青年失业率", "max_results": 20, "search_depth": "advanced", "topic": "news",
            "include_answer": false, "include_raw_content": false, "include_images": false,
            "time_range": "month", "include_domains": ["stats.example.org"], "include_usage": true
        })
    );
    // 回复里没有点数：交 None，由调用方按报价记
    let fake = Fake::new().reply(
        Method::Post,
        search_url(),
        json_reply(200, r#"{"results": []}"#),
    );
    let reply = Client::new("k", &fake)
        .search(
            &Search {
                query: "q".into(),
                ..Default::default()
            },
            &Cancel::new(),
        )
        .unwrap();
    assert!(reply.value.is_empty() && reply.credits.is_none());
}

#[test]
fn extract_crawl_and_map() {
    let fake = Fake::new()
        .reply(
            Method::Post,
            "https://api.tavily.com/extract",
            json_reply(
                200,
                r##"{"results": [{"url": "https://a.example.org/p", "raw_content": "# 标题\n\n正文"}],
                    "failed_results": [{"url": "https://b.example.org/q", "error": "timeout"}],
                    "usage": {"credits": 0}}"##,
            ),
        )
        .reply(
            Method::Post,
            "https://api.tavily.com/crawl",
            json_reply(
                200,
                r#"{"base_url": "https://docs.example.org", "results": [
                    {"url": "https://docs.example.org/a", "raw_content": "A 页"},
                    {"url": "https://docs.example.org/b", "raw_content": "B 页"}],
                    "usage": {"credits": 3}}"#,
            ),
        )
        .reply(
            Method::Post,
            "https://api.tavily.com/map",
            json_reply(
                200,
                r#"{"base_url": "https://docs.example.org", "results": ["https://docs.example.org/a", "https://docs.example.org/b"], "usage": {"credits": 1}}"#,
            ),
        );
    let client = Client::new("k", &fake);
    let cancel = Cancel::new();
    let got = client
        .extract(
            &[
                "https://a.example.org/p".into(),
                "https://b.example.org/q".into(),
            ],
            Depth::Basic,
            &cancel,
        )
        .unwrap();
    assert_eq!(got.value.pages[0].content, "# 标题\n\n正文");
    assert_eq!(got.value.failed, ["https://b.example.org/q"]);
    assert_eq!(got.credits, Some(0.0), "不足 5 页时 Tavily 可能报 0 点");
    let c = Crawl {
        url: "https://docs.example.org".into(),
        max_depth: 9,
        limit: 10,
        instructions: Some("只要 API 文档".into()),
        select_paths: vec!["/docs/.*".into()],
        extract_depth: Depth::Advanced,
    };
    let crawled = client.crawl(&c, &cancel).unwrap();
    assert_eq!((crawled.value.len(), crawled.credits), (2, Some(3.0)));
    let mapped = client.map(&c, &cancel).unwrap();
    assert_eq!(
        mapped.value,
        ["https://docs.example.org/a", "https://docs.example.org/b"]
    );
    let seen = fake.requests();
    assert_eq!(
        seen[0].json()["urls"],
        json!(["https://a.example.org/p", "https://b.example.org/q"])
    );
    assert_eq!(seen[0].json()["format"], "markdown");
    let crawl = seen[1].json();
    assert_eq!(
        (
            &crawl["max_depth"],
            &crawl["allow_external"],
            &crawl["extract_depth"],
            &crawl["instructions"],
            &crawl["select_paths"]
        ),
        (
            &json!(5),
            &json!(false),
            &json!("advanced"),
            &json!("只要 API 文档"),
            &json!(["/docs/.*"])
        )
    );
    assert!(
        seen[2].json().get("extract_depth").is_none(),
        "地图不取正文"
    );
}

#[test]
fn errors_by_status() {
    let cancel = Cancel::new();
    let search = Search {
        query: "q".into(),
        ..Default::default()
    };
    let crawl = Crawl {
        url: "https://x.example.org".into(),
        ..Default::default()
    };
    for (status, endpoint, want) in [
        (401, "search", Error::Auth),
        (403, "search", Error::Auth),
        (403, "crawl", Error::UnsupportedSite),
        (400, "search", Error::BadRequest),
        (422, "search", Error::BadRequest),
        (432, "search", Error::Quota),
        (433, "crawl", Error::Quota),
        (418, "search", Error::Service),
    ] {
        let fake = Fake::new().reply(
            Method::Post,
            &format!("https://api.tavily.com/{endpoint}"),
            json_reply(
                status,
                r#"{"detail": {"error": "Unauthorized: missing or invalid API key tvly-secret"}}"#,
            ),
        );
        let client = Client::new("tvly-secret", &fake);
        let got = if endpoint == "crawl" {
            client.crawl(&crawl, &cancel).map(|_| ())
        } else {
            client.search(&search, &cancel).map(|_| ())
        };
        let e = got.unwrap_err();
        assert_eq!(e, want, "{status} {endpoint}");
        assert!(
            !e.to_string().contains("tvly-secret"),
            "错误里不带密钥和服务原文"
        );
        assert_eq!(fake.requests().len(), 1, "{status} 不重试");
    }
    // 200 但读不懂
    let fake = Fake::new().reply(Method::Post, search_url(), json_reply(200, "<html>"));
    assert_eq!(
        Client::new("k", &fake)
            .search(&search, &cancel)
            .unwrap_err(),
        Error::Service
    );
    // 没有密钥：一个请求都不发
    let fake = Fake::new();
    assert_eq!(
        Client::new(" ", &fake)
            .search(&search, &cancel)
            .unwrap_err(),
        Error::Auth
    );
    assert!(fake.requests().is_empty());
}

#[test]
fn rate_limits_and_server_errors_are_retried() {
    let cancel = Cancel::new();
    let search = Search {
        query: "q".into(),
        ..Default::default()
    };
    let busy =
        || json_reply(429, r#"{"detail": {"error": "rate limit"}}"#).header("retry-after", "0");
    let fake = Fake::new()
        .reply(Method::Post, search_url(), busy())
        .reply(
            Method::Post,
            search_url(),
            json_reply(500, "{}").header("retry-after", "0"),
        )
        .reply(Method::Post, search_url(), json_reply(200, SEARCH));
    let got = Client::new("k", &fake).search(&search, &cancel).unwrap();
    assert_eq!((got.value.len(), fake.requests().len()), (2, 3));
    // 重试 3 次还不行
    let fake = Fake::new().reply(Method::Post, search_url(), busy());
    assert_eq!(
        Client::new("k", &fake)
            .search(&search, &cancel)
            .unwrap_err(),
        Error::RateLimited
    );
    assert_eq!(fake.requests().len(), 4, "一次加重试 3 次");
    let fake = Fake::new().reply(
        Method::Post,
        search_url(),
        json_reply(503, "{}").header("retry-after", "0"),
    );
    assert_eq!(
        Client::new("k", &fake)
            .search(&search, &cancel)
            .unwrap_err(),
        Error::Service
    );
    // 等的时候按了停
    let fake = Fake::new().reply(
        Method::Post,
        search_url(),
        json_reply(429, "{}").header("retry-after", "20"),
    );
    let stop = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        stop.cancel();
    });
    let start = Instant::now();
    assert_eq!(
        Client::new("k", &fake)
            .search(&search, &cancel)
            .unwrap_err(),
        Error::Cancelled
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}
