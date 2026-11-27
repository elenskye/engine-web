//! 抓取的规矩（模拟服务，不连外网）：跳转、内网、大小、取消、Cookie。Peck `web.rs`、Pith `download.rs` 的抓取测试搬到这里。
#![cfg(feature = "fake")]

use engine_web::engine_core::Cancel;
use engine_web::engine_core::call::Zeroizing;
use engine_web::transport::{Method, Response};
use engine_web::{Error, Fake, FetchOptions, HostPacer, fetch};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn opts() -> FetchOptions {
    FetchOptions::default()
}

#[test]
fn pages_come_back_with_the_final_url_and_type() {
    let fake = Fake::new()
        .redirect("https://news.example.com/old", "/new")
        .redirect(
            "https://news.example.com/new",
            "https://www.news.example.com/final",
        )
        .page(
            "https://www.news.example.com/final",
            "text/html; charset=gbk",
            "<p>到了</p>",
        );
    let got = fetch(
        "https://news.example.com/old",
        &opts(),
        &fake,
        &Cancel::new(),
    )
    .unwrap();
    assert_eq!(got.url, "https://www.news.example.com/final");
    assert_eq!(got.content_type.as_deref(), Some("text/html; charset=gbk"));
    assert_eq!(got.bytes, "<p>到了</p>".as_bytes());
    assert_eq!(
        got.redirects,
        [
            "https://news.example.com/old",
            "https://news.example.com/new"
        ]
    );
    let seen = fake.requests();
    assert_eq!(seen.len(), 3);
    assert!(
        seen[0].header("accept").unwrap().starts_with("text/html"),
        "默认的 Accept"
    );
    assert_eq!(
        seen[0].pinned,
        ["93.184.216.34:443".parse().unwrap()],
        "检查过的地址钉住再连"
    );
}

#[test]
fn redirects_to_other_sites_stop_with_the_new_url() {
    let fake = Fake::new()
        .redirect(
            "https://a.example.com/away",
            "https://b.example.org/elsewhere",
        )
        .redirect("https://a.example.com/down", "http://a.example.com/plain")
        .redirect("https://a.example.com/loop", "/loop")
        .reply(
            Method::Get,
            "https://a.example.com/nowhere",
            Response::new(302),
        );
    let cancel = Cancel::new();
    assert_eq!(
        fetch("https://a.example.com/away", &opts(), &fake, &cancel).unwrap_err(),
        Error::RedirectElsewhere {
            to: "https://b.example.org/elsewhere".into()
        }
    );
    assert!(
        matches!(
            fetch("https://a.example.com/down", &opts(), &fake, &cancel),
            Err(Error::RedirectElsewhere { .. })
        ),
        "https → http 不跟"
    );
    assert_eq!(
        fetch("https://a.example.com/loop", &opts(), &fake, &cancel).unwrap_err(),
        Error::TooManyRedirects
    );
    assert_eq!(
        fake.requests()
            .iter()
            .filter(|s| s.url.ends_with("/loop"))
            .count(),
        6,
        "跟 5 次（一共请求 6 次）"
    );
    assert_eq!(
        fetch("https://a.example.com/nowhere", &opts(), &fake, &cancel).unwrap_err(),
        Error::HttpStatus { status: 302 },
        "要跳转但没说跳到哪"
    );
}

#[test]
fn other_sites_can_be_followed_without_the_apps_cookie() {
    let fake = Fake::new()
        .redirect("https://a.example.com/s", "https://b.example.org/t")
        .page("https://b.example.org/t", "text/plain", "到了");
    let opts = FetchOptions {
        follow_other_sites: true,
        headers: vec![
            ("Cookie".into(), Zeroizing::new("sid=secret".into())),
            ("Accept-Language".into(), Zeroizing::new("zh-CN".into())),
        ],
        ..opts()
    };
    let got = fetch("https://a.example.com/s", &opts, &fake, &Cancel::new()).unwrap();
    assert_eq!(got.url, "https://b.example.org/t");
    let seen = fake.requests();
    assert_eq!(seen[0].header("cookie"), Some("sid=secret"));
    assert_eq!(seen[1].header("cookie"), None, "到别的网站不带 Cookie");
    assert_eq!(seen[1].header("accept-language"), Some("zh-CN"));
}

#[test]
fn local_and_private_hosts_are_refused() {
    let fake = Fake::new()
        .host("intranet.example.com", "10.0.0.5")
        .host("rebind.example.com", "127.0.0.1")
        .host("gone.example.com", "")
        .redirect("https://public.example.com/r", "http://127.0.0.1/admin");
    let cancel = Cancel::new();
    for url in [
        "http://localhost:5173/",
        "http://dev.localhost/",
        "http://printer.local/",
        "http://db.internal/",
        "http://127.0.0.1:8080/",
        "http://[::1]/",
        "http://10.0.0.5/",
        "http://169.254.169.254/latest/meta-data/",
        "https://intranet.example.com/",
        "https://rebind.example.com/",
    ] {
        assert_eq!(
            fetch(url, &opts(), &fake, &cancel).unwrap_err(),
            Error::PrivateAddress,
            "{url}"
        );
    }
    assert_eq!(
        fetch("https://public.example.com/r", &opts(), &fake, &cancel).unwrap_err(),
        Error::RedirectElsewhere {
            to: "http://127.0.0.1/admin".into()
        }
    );
    let follow = FetchOptions {
        follow_other_sites: true,
        ..opts()
    };
    assert_eq!(
        fetch("https://public.example.com/r", &follow, &fake, &cancel).unwrap_err(),
        Error::PrivateAddress,
        "跟到别的网站也每一跳都查"
    );
    assert_eq!(
        fetch("https://gone.example.com/", &opts(), &fake, &cancel).unwrap_err(),
        Error::UnknownHost
    );
    assert!(
        fake.requests().iter().all(|s| !s.url.contains("127.0.0.1")
            && !s.url.contains("intranet")
            && !s.url.contains("localhost")),
        "内网地址一个请求都不发"
    );
    // App 放开以后照常抓，不钉地址
    let open = FetchOptions {
        allow_private: true,
        ..opts()
    };
    let fake = fake.page("http://localhost:5173/", "text/plain", "dev server");
    assert_eq!(
        fetch("http://localhost:5173/", &open, &fake, &cancel)
            .unwrap()
            .bytes,
        b"dev server"
    );
    assert!(fake.requests().last().unwrap().pinned.is_empty());
}

#[test]
fn bad_urls_statuses_and_network_errors() {
    let fake = Fake::new()
        .reply(
            Method::Get,
            "https://x.example.com/gone",
            Response::with(404, "text/html", "<h1>没有</h1>"),
        )
        .fail(Method::Get, "https://x.example.com/slow", Error::Timeout);
    let cancel = Cancel::new();
    for url in [
        "ftp://x.example.com/",
        "https://u:p@x.example.com/",
        "x.example.com/a",
    ] {
        assert_eq!(
            fetch(url, &opts(), &fake, &cancel).unwrap_err(),
            Error::BadUrl,
            "{url}"
        );
    }
    assert_eq!(
        fetch("https://x.example.com/gone", &opts(), &fake, &cancel).unwrap_err(),
        Error::HttpStatus { status: 404 }
    );
    assert_eq!(
        fetch("https://x.example.com/slow", &opts(), &fake, &cancel).unwrap_err(),
        Error::Timeout
    );
}

#[test]
fn big_pages_are_cut_and_limits_stay_within_the_engines() {
    let fake = Fake::new().page("https://x.example.com/big", "text/plain", vec![b'a'; 5000]);
    let small = FetchOptions {
        max_bytes: 1000,
        ..opts()
    };
    let got = fetch("https://x.example.com/big", &small, &fake, &Cancel::new()).unwrap();
    assert_eq!((got.bytes.len(), got.truncated), (1000, true));
    let huge = FetchOptions {
        max_bytes: u64::MAX,
        max_redirects: 99,
        timeout: Duration::from_secs(3600),
        ..opts()
    };
    let fake = fake.redirect("https://x.example.com/loop", "/loop");
    assert_eq!(
        fetch("https://x.example.com/loop", &huge, &fake, &Cancel::new()).unwrap_err(),
        Error::TooManyRedirects
    );
    assert_eq!(
        fake.requests()
            .iter()
            .filter(|s| s.url.ends_with("/loop"))
            .count(),
        11,
        "跳转最多 10 次（引擎的上限）"
    );
}

#[test]
fn fetching_stops_when_cancelled() {
    let fake = Fake::new().page("https://x.example.com/", "text/plain", "x");
    let cancel = Cancel::new();
    cancel.cancel();
    assert_eq!(
        fetch("https://x.example.com/", &opts(), &fake, &cancel).unwrap_err(),
        Error::Cancelled
    );
    assert!(fake.requests().is_empty());
}

#[test]
fn same_site_requests_are_spaced_out() {
    let fake = Fake::new()
        .page("https://x.example.com/1", "text/plain", "1")
        .page("https://www.x.example.com/2", "text/plain", "2");
    let opts = FetchOptions {
        pacer: Some(Arc::new(HostPacer::new(Duration::from_millis(150)))),
        ..opts()
    };
    let start = Instant::now();
    fetch("https://x.example.com/1", &opts, &fake, &Cancel::new()).unwrap();
    fetch("https://www.x.example.com/2", &opts, &fake, &Cancel::new()).unwrap();
    assert!(
        start.elapsed() >= Duration::from_millis(140),
        "www. 算同一站"
    );
}
