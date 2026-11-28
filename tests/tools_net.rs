//! 联网的工具（`web_fetch`、只给网址的 `web_find`、`web_search`、`web_crawl`、`web_map`）：模拟服务上跑清单里的每个例子，
//! 再核对报价、确认、用量、没有密钥、命令行。不连外网、不花钱。
#![cfg(feature = "fake")]

use engine_web::Fake;
use engine_web::engine_core::Cancel;
use engine_web::engine_core::call::{Ctx, Keys, Prices, Zeroizing};
use engine_web::engine_core::cli::{LocalHost, run_with};
use engine_web::tools::{self, Web};
use engine_web::transport::{Method, Response};
use serde_json::{Value, json};
use std::sync::Arc;

const ARTICLE: &str = "https://news.example.com/city/2026/line5-south.html";

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("tests/fixtures/html/{name}")).unwrap()
}

fn tavily(endpoint: &str, body: Value) -> (String, Response) {
    (
        format!("https://api.tavily.com/{endpoint}"),
        Response::with(200, "application/json", body.to_string()),
    )
}

/// 分三页的文章第 `n` 页的网址。
fn story(n: u32) -> String {
    if n == 1 {
        "https://news.example.com/city/2026/metro-history.html".into()
    } else {
        format!("https://news.example.com/city/2026/metro-history_{n}.html")
    }
}

/// 清单例子用到的网址都在这里。
fn fake() -> Fake {
    let mut fake = Fake::new()
        .page(ARTICLE, "text/html; charset=utf-8", fixture("zh-news.html"))
        .reply(
            Method::Get,
            "https://news.example.com/old/line5.html",
            Response::new(301).header("location", "/city/2026/line5-south.html"),
        )
        .redirect(
            "https://news.example.com/go/partner",
            "https://partner.example.org/line5",
        )
        .page(
            "https://papers.example.org/line5-study.pdf",
            "application/pdf",
            fixture("paper.pdf"),
        )
        .page(
            "https://app.example.com/dashboard",
            "text/html",
            fixture("spa-shell.html"),
        )
        .page(
            "https://shop.example.com/item/42",
            "text/html; charset=UTF-8",
            fixture("bot-check.html"),
        );
    for n in 1..=3 {
        fake = fake.page(
            &story(n),
            "text/html; charset=utf-8",
            fixture(&format!("story-{n}.html")),
        );
    }
    for (url, resp) in [
        tavily(
            "extract",
            json!({"results": [{"url": "https://app.example.com/dashboard", "raw_content": "# 仪表盘\n\n本月新增用户 1,280 人，比上月多 12%。"}], "failed_results": [], "usage": {"credits": 1}}),
        ),
        tavily(
            "search",
            json!({"query": "q", "results": [
                {"title": "示例市地铁五号线南延段通车", "url": ARTICLE, "content": "南延段全长 12.6 公里，设 8 座车站……", "score": 0.93, "published_date": "2026-11-20"},
                {"title": "五号线换乘指南", "url": "https://news.example.com/city/2026/metro-guide.html", "content": "换乘站一览……", "score": 0.71}
            ], "usage": {"credits": 1}, "request_id": "r-1"}),
        ),
        tavily(
            "crawl",
            json!({"base_url": "https://docs.example.org", "results": [
                {"url": "https://docs.example.org/", "raw_content": "# 文档首页\n\n从这里开始。"},
                {"url": "https://docs.example.org/install", "raw_content": "# 安装\n\n运行 `heron new`。"}
            ], "usage": {"credits": 2}}),
        ),
        tavily(
            "map",
            json!({"base_url": "https://docs.example.org", "results": [
                "https://docs.example.org/", "https://docs.example.org/install", "https://docs.example.org/config"
            ], "usage": {"credits": 1}}),
        ),
    ] {
        fake = fake.reply(Method::Post, &url, resp);
    }
    fake
}

struct TestKeys(Option<&'static str>);

impl Keys for TestKeys {
    fn get(&self, name: &str) -> Option<Zeroizing<String>> {
        (name == "tavily")
            .then_some(self.0)
            .flatten()
            .map(|k| Zeroizing::new(k.to_string()))
    }
}

const KEY: TestKeys = TestKeys(Some("tvly-test"));
const NO_KEY: TestKeys = TestKeys(None);

fn with_keys<'a>(host: &'a LocalHost, cancel: &'a Cancel, keys: &'a TestKeys) -> Ctx<'a> {
    Ctx {
        keys,
        ..host.ctx(cancel)
    }
}

fn toolkit(fake: Arc<Fake>) -> engine_web::engine_core::call::Toolkit<Web> {
    tools::toolkit_with(Web::new(fake))
}

#[test]
fn every_example_runs_against_the_fake_services() {
    let host = LocalHost { yes: true };
    let cancel = Cancel::new();
    toolkit(Arc::new(fake()))
        .check_examples(&with_keys(&host, &cancel, &KEY))
        .unwrap();
}

#[test]
fn quotes_and_confirmation_follow_the_manifest() {
    let tk = toolkit(Arc::new(fake()));
    let host = LocalHost::default();
    let cancel = Cancel::new();
    let ctx = with_keys(&host, &cancel, &KEY);
    let quote = |name: &str, args: Value| {
        let p = tk.prepare(name, &args, &ctx).unwrap();
        (p.quote().map(|q| q.units), p.needs_confirm())
    };
    assert_eq!(
        quote("web_fetch", json!({"url": ARTICLE})),
        (Some(0.0), false),
        "本机抓不花钱"
    );
    assert_eq!(
        quote("web_fetch", json!({"url": ARTICLE, "via": "auto"})),
        (Some(1.0), false)
    );
    assert_eq!(
        quote(
            "web_fetch",
            json!({"url": ARTICLE, "via": "tavily", "depth": "advanced"})
        ),
        (Some(2.0), false)
    );
    assert_eq!(
        quote("web_search", json!({"query": "q"})),
        (Some(1.0), false)
    );
    assert_eq!(
        quote("web_search", json!({"query": "q", "depth": "advanced"})),
        (Some(2.0), false)
    );
    assert_eq!(
        quote("web_crawl", json!({"url": "https://docs.example.org/"})),
        (Some(3.0), true),
        "默认 10 页"
    );
    assert_eq!(
        quote(
            "web_crawl",
            json!({"url": "https://docs.example.org/", "limit": 50, "instructions": "只要安装", "extract_depth": "advanced"})
        ),
        (Some(30.0), true)
    );
    assert_eq!(
        quote("web_map", json!({"url": "https://docs.example.org/"})),
        (Some(5.0), true)
    );
    let p = tk
        .prepare(
            "web_crawl",
            &json!({"url": "https://docs.example.org/"}),
            &ctx,
        )
        .unwrap();
    assert!((p.quote().unwrap().usd - 0.024).abs() < 1e-12);
    // 上限照清单
    assert_eq!(
        tk.prepare(
            "web_crawl",
            &json!({"url": "https://docs.example.org/", "limit": 51}),
            &ctx
        )
        .unwrap_err()
        .kind,
        "bad_args"
    );
}

#[test]
fn usage_follows_the_credits_tavily_reports() {
    let tk = toolkit(Arc::new(fake()));
    let host = LocalHost { yes: true };
    let cancel = Cancel::new();
    let mut prices = Prices::new();
    prices.set("tavily", "credit", 0.01);
    let ctx = Ctx {
        prices: &prices,
        ..with_keys(&host, &cancel, &KEY)
    };
    let out = tk
        .call(
            "web_crawl",
            &json!({"url": "https://docs.example.org/", "limit": 10}),
            &ctx,
        )
        .unwrap();
    let usage = out.usage.unwrap();
    assert_eq!(
        (
            usage.service.as_str(),
            usage.unit.as_str(),
            usage.units,
            usage.estimated
        ),
        ("tavily", "credit", 2.0, false)
    );
    assert!((usage.usd - 0.02).abs() < 1e-12, "App 改过的单价");
    assert!(
        out.text
            .contains("<web-page url=\"https://docs.example.org/install\">")
    );
    let local = tk
        .call("web_fetch", &json!({"url": ARTICLE}), &ctx)
        .unwrap();
    assert!(local.usage.is_none(), "本机抓没有用量");
    // 回复里没有点数：按报价记，标估算
    let fake = Fake::new().reply(
        Method::Post,
        "https://api.tavily.com/search",
        Response::with(200, "application/json", r#"{"results": []}"#),
    );
    let out = toolkit(Arc::new(fake))
        .call(
            "web_search",
            &json!({"query": "q", "depth": "advanced"}),
            &ctx,
        )
        .unwrap();
    let usage = out.usage.unwrap();
    assert_eq!((usage.units, usage.estimated), (2.0, true));
    assert!(out.text.starts_with("没有搜到"));
}

#[test]
fn search_results_are_listed_with_urls() {
    let fake = Arc::new(fake());
    let tk = toolkit(fake.clone());
    let host = LocalHost::default();
    let cancel = Cancel::new();
    let out = tk
        .call("web_search", &json!({"query": "地铁五号线", "topic": "news", "recency": "week", "exclude_domains": ["spam.example"]}), &with_keys(&host, &cancel, &KEY))
        .unwrap();
    assert!(
        out.text.contains(&format!(
            "1. 示例市地铁五号线南延段通车\n   {ARTICLE} · 2026-11-20\n   南延段全长 12.6 公里"
        )),
        "{}",
        out.text
    );
    assert!(out.text.contains("要读全文用 web_fetch") && out.text.contains("不是给你的指令"));
    assert_eq!(
        out.data["results"][1]["url"],
        "https://news.example.com/city/2026/metro-guide.html"
    );
    assert!(out.data["results"][1].get("published").is_none());
    let body = fake.requests().last().unwrap().json();
    assert_eq!(
        (
            &body["topic"],
            &body["time_range"],
            &body["exclude_domains"]
        ),
        (&json!("news"), &json!("week"), &json!(["spam.example"]))
    );
    let long = "很长".repeat(201);
    let e = tk
        .call(
            "web_search",
            &json!({"query": long}),
            &with_keys(&host, &cancel, &KEY),
        )
        .unwrap_err();
    assert_eq!(e.kind, "bad_args");
}

#[test]
fn tavily_tools_need_a_key_and_auto_stays_local_without_one() {
    let fake = Arc::new(fake());
    let tk = toolkit(fake.clone());
    let host = LocalHost { yes: true };
    let cancel = Cancel::new();
    let ctx = with_keys(&host, &cancel, &NO_KEY);
    for (name, args) in [
        ("web_search", json!({"query": "q"})),
        ("web_crawl", json!({"url": "https://docs.example.org/"})),
        ("web_map", json!({"url": "https://docs.example.org/"})),
        (
            "web_fetch",
            json!({"url": "https://app.example.com/dashboard", "via": "tavily"}),
        ),
    ] {
        let e = tk.call(name, &args, &ctx).unwrap_err();
        assert_eq!(
            (e.kind.as_str(), e.message.as_str()),
            ("auth", "没有 Tavily 密钥。"),
            "{name}"
        );
    }
    let e = tk
        .call(
            "web_fetch",
            &json!({"url": "https://app.example.com/dashboard", "via": "auto"}),
            &ctx,
        )
        .unwrap_err();
    assert_eq!(e.kind, "no_article", "没有密钥就不改用 Tavily");
    assert!(e.hint.contains("via: tavily"));
    assert!(
        fake.requests()
            .iter()
            .all(|s| !s.url.starts_with("https://api.tavily.com")),
        "一个 Tavily 请求都没发"
    );
    // 有密钥时 auto 改用 Tavily，写明本机为什么没成，记用量
    let ctx = with_keys(&host, &cancel, &KEY);
    let out = tk
        .call(
            "web_fetch",
            &json!({"url": "https://app.example.com/dashboard", "via": "auto"}),
            &ctx,
        )
        .unwrap();
    assert_eq!(
        (&out.data["via"], &out.data["fallback_from"]),
        (&json!("tavily"), &json!("这个网页里没读到正文。"))
    );
    assert!(out.text.contains("本月新增用户 1,280 人") && out.text.contains("本机抓没成"));
    assert_eq!(out.usage.unwrap().units, 1.0);
    let req = fake
        .requests()
        .into_iter()
        .find(|s| s.url.ends_with("/extract"))
        .unwrap();
    assert_eq!(req.header("authorization"), Some("Bearer tvly-test"));
    // 内网、跳到别的网站：auto 也不改用 Tavily
    let e = tk
        .call(
            "web_fetch",
            &json!({"url": "https://news.example.com/go/partner", "via": "auto"}),
            &ctx,
        )
        .unwrap_err();
    assert_eq!(e.kind, "redirect_elsewhere");
    assert!(
        e.message.contains("https://partner.example.org/line5"),
        "{}",
        e.message
    );
}

#[test]
fn fetch_hands_back_non_pages_as_attachments_and_pages_through_long_ones() {
    let tk = toolkit(Arc::new(fake()));
    let host = LocalHost::default();
    let cancel = Cancel::new();
    let ctx = with_keys(&host, &cancel, &NO_KEY);
    let pdf = tk
        .call(
            "web_fetch",
            &json!({"url": "https://papers.example.org/line5-study.pdf"}),
            &ctx,
        )
        .unwrap();
    assert_eq!(pdf.attachments.len(), 1);
    assert_eq!(
        (
            pdf.attachments[0].filename.as_str(),
            pdf.attachments[0].mime_type.as_str()
        ),
        ("line5-study.pdf", "application/pdf")
    );
    assert!(
        pdf.text.starts_with("这是 PDF，不是网页。原件（")
            && pdf.text.contains("请把这份文件交给 PDF 工具读。")
    );
    let first = tk
        .call(
            "web_fetch",
            &json!({"url": ARTICLE, "max_chars": 500}),
            &ctx,
        )
        .unwrap();
    let next = first.next.clone().expect("没交完要给 next");
    assert_eq!(next["offset"], first.data["end"]);
    let second = tk.call("web_fetch", &next, &ctx).unwrap();
    assert_eq!(second.data["offset"], first.data["end"]);
    let e = tk
        .call(
            "web_fetch",
            &json!({"url": "https://news.example.com/missing"}),
            &ctx,
        )
        .unwrap_err();
    assert_eq!(
        (e.kind.as_str(), e.message.as_str()),
        ("http_status", "网站回了 HTTP 404。")
    );
    assert!(e.hint.contains("网址可能错了"));
    // web_find 只给网址：现抓一次
    let found = tk
        .call(
            "web_find",
            &json!({"url": ARTICLE, "query": "全长１２.６公里"}),
            &ctx,
        )
        .unwrap();
    assert_eq!(found.data["total"], 1);
}

#[test]
fn cli_asks_for_yes_before_spending() {
    let tk = toolkit(Arc::new(fake()));
    let run = |args: &[&str]| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(&tk, args, &mut out, &mut err);
        (
            code,
            serde_json::from_slice::<Value>(&out).unwrap_or(Value::Null),
        )
    };
    let (code, reply) = run(&[
        "call",
        "web_crawl",
        r#"{"url":"https://docs.example.org/"}"#,
    ]);
    assert_eq!(code, 3, "不加 --yes 只报价");
    assert_eq!(
        (&reply["quote"]["units"], &reply["quote"]["service"]),
        (&json!(3.0), &json!("tavily"))
    );
    let (code, reply) = run(&["call", "web_search", r#"{"query":"q"}"#]);
    assert_eq!(
        (code, &reply["quote"]["units"]),
        (3, &json!(1.0)),
        "命令行对一切花钱的都要 --yes"
    );
    // 本机抓报价 0：不用 --yes
    let (code, reply) = run(&[
        "call",
        "web_fetch",
        &format!(r#"{{"url":"{ARTICLE}"}}"#),
        "--json-only",
    ]);
    assert_eq!((code, &reply["via"]), (0, &json!("local")));
}

#[test]
fn multi_page_articles_are_merged_and_stop_on_failures() {
    let host = LocalHost::default();
    let cancel = Cancel::new();
    let ctx = with_keys(&host, &cancel, &KEY);
    let fake = Arc::new(fake());
    let tk = toolkit(fake.clone());
    let out = tk
        .call("web_fetch", &json!({"url": story(1)}), &ctx)
        .unwrap();
    assert_eq!(out.data["pages"], json!([story(1), story(2), story(3)]));
    assert!(out.text.contains(&format!("第 2 页：{}", story(2))));
    assert!(out.text.contains("1993 年 10 月") && out.text.contains("2032 年以前"));
    assert!(out.text.contains("文中的图（3 张"), "{}", out.text);
    assert!(out.text.contains(
        "- 第 2 页第 3 段后：二号线通车那天的站台 https://news.example.com/img/2026/metro-2004.jpg"
    ));
    assert_eq!(out.data["images"][1]["page"], 2);
    assert!(out.data.get("stopped").is_none());

    // 上限：只合两页，写明后面还有
    let two = tk
        .call("web_fetch", &json!({"url": story(1), "max_pages": 2}), &ctx)
        .unwrap();
    assert_eq!(two.data["pages"].as_array().unwrap().len(), 2);
    assert_eq!(
        two.data["stopped"],
        json!({"reason": "limit", "next": story(3)})
    );
    assert!(two.text.contains(&format!("后面还有：{}", story(3))));

    // 第 3 页取不到：交回前两页、写明哪一页为什么
    let broken = Arc::new(fake_without_page_three());
    let out = toolkit(broken)
        .call("web_fetch", &json!({"url": story(1)}), &ctx)
        .unwrap();
    assert_eq!(out.data["pages"].as_array().unwrap().len(), 2);
    assert_eq!(out.data["stopped"]["reason"], "failed");
    assert_eq!(out.data["stopped"]["error"], "http_status");
    assert!(
        out.text
            .contains(&format!("{} 没取到（网站回了 HTTP 500）", story(3))),
        "{}",
        out.text
    );

    // web_find 也接着抓
    let found = tk
        .call("web_find", &json!({"url": story(1), "query": "九百"}), &ctx)
        .unwrap();
    assert!(found.text.starts_with("没有找到"));
    let found = tk
        .call(
            "web_find",
            &json!({"url": story(1), "query": "一百万人次"}),
            &ctx,
        )
        .unwrap();
    assert!(
        found.text.contains("第 2 页〈二号线和三号线：织成一张网〉"),
        "{}",
        found.text
    );
    assert_eq!(found.data["hits"][0]["location"]["page"], 2);
}

/// 第 3 页回 500 的模拟服务。
fn fake_without_page_three() -> Fake {
    let mut fake = Fake::new();
    for n in 1..=2 {
        fake = fake.page(
            &story(n),
            "text/html; charset=utf-8",
            fixture(&format!("story-{n}.html")),
        );
    }
    fake.reply(Method::Get, &story(3), Response::new(500))
}

#[test]
fn bot_check_pages_are_errors_and_auto_falls_back_to_tavily() {
    let host = LocalHost::default();
    let cancel = Cancel::new();
    let ctx = with_keys(&host, &cancel, &KEY);
    let tk = toolkit(Arc::new(fake()));
    let err = tk
        .call(
            "web_fetch",
            &json!({"url": "https://shop.example.com/item/42"}),
            &ctx,
        )
        .unwrap_err();
    assert_eq!(err.kind, "bot_check");
    assert!(err.message.contains("Cloudflare"), "{}", err.message);
    assert!(err.hint.contains("via: tavily"));
    let out = tk
        .call(
            "web_fetch",
            &json!({"url": "https://shop.example.com/item/42", "via": "auto"}),
            &ctx,
        )
        .unwrap();
    assert_eq!(out.data["via"], "tavily");
    assert_eq!(
        out.data["fallback_from"],
        "网站回的是挡程序的验证页（Cloudflare），不是正文。"
    );
}
