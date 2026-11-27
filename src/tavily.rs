//! Tavily 的四个接口：搜索 `/search`、取页 `/extract`、爬站 `/crawl`、站内地图 `/map`。规矩见 `docs/design.md` §5，
//! 参数和计点照官方文档（2026-11-27 核对，`docs/decisions/0007`）。
//!
//! - 每个请求都带 `include_usage: true`，回复里的 `usage.credits` 就是这次花的点数（[`Reply::credits`]）；没有就是 `None`，
//!   由调用方按报价记、标「估算」。
//! - 本地报价（[`quote`]）按最坏情况算，不联网。
//! - 出错按状态码分（[`Error`]）：401 → `Auth`；403 → 搜索、取页是 `Auth`，爬站、地图是 `UnsupportedSite`；
//!   400、422 → `BadRequest`；432、433 → `Quota`；429、434 → `RateLimited`；5xx → `Service`。
//!   429、434 和 5xx 按 `Retry-After` 等了再试，最多 3 次，等的时候可以取消。服务原文不交出去（可能带密钥、看不懂）。

use crate::Error;
use crate::transport::{Request, Transport};
use engine_core::Cancel;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// Tavily 的网址。
pub const BASE: &str = "https://api.tavily.com";
/// 按量付费每点的价钱（官方价目，2026-11-27）。App 可以用 `Prices` 改。
pub const USD_PER_CREDIT: f64 = 0.008;
/// 重试前等多久（没有 `Retry-After` 时）。
const WAITS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(3),
    Duration::from_secs(8),
];
/// `Retry-After` 最多等这么久。
const MAX_WAIT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Depth {
    #[default]
    Basic,
    Advanced,
}

impl Depth {
    pub fn key(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Advanced => "advanced",
        }
    }

    pub fn parse(s: &str) -> Self {
        if s == "advanced" {
            Self::Advanced
        } else {
            Self::Basic
        }
    }

    fn factor(self) -> f64 {
        match self {
            Self::Basic => 1.0,
            Self::Advanced => 2.0,
        }
    }
}

/// 一次搜索。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Search {
    pub query: String,
    /// 1–20，默认 5。
    pub max_results: u32,
    pub depth: Depth,
    /// `general`（默认）或 `news`。
    pub topic: String,
    /// `day`、`week`、`month`、`year`。
    pub recency: Option<String>,
    pub domains: Vec<String>,
    pub exclude_domains: Vec<String>,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            query: String::new(),
            max_results: 5,
            depth: Depth::Basic,
            topic: "general".into(),
            recency: None,
            domains: Vec::new(),
            exclude_domains: Vec::new(),
        }
    }
}

/// 一条搜索结果。
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub title: String,
    pub url: String,
    /// Tavily 给的摘录。
    pub content: String,
    pub published: Option<String>,
    pub score: f64,
}

/// 一页正文（取页、爬站）。`content` 是 Tavily 转好的 Markdown。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub url: String,
    pub content: String,
}

/// 取页的结果：取到的页和没取到的网址。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extracted {
    pub pages: Vec<Page>,
    pub failed: Vec<String>,
}

/// 爬站或站内地图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crawl {
    pub url: String,
    /// 1–5，默认 1。
    pub max_depth: u32,
    /// 最多几页，默认 10。
    pub limit: u32,
    /// 用自然语言说要哪些页（点数翻倍）。
    pub instructions: Option<String>,
    /// 只要这些路径（正则，如 `/docs/.*`）。
    pub select_paths: Vec<String>,
    /// 爬站时取正文的深度（地图不用）。
    pub extract_depth: Depth,
}

impl Default for Crawl {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_depth: 1,
            limit: 10,
            instructions: None,
            select_paths: Vec::new(),
            extract_depth: Depth::Basic,
        }
    }
}

/// 一次调用的结果和 Tavily 报的点数（回复里没有就是 `None`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Reply<T> {
    pub value: T,
    pub credits: Option<f64>,
    /// Tavily 的请求编号（对账用）。
    pub request_id: Option<String>,
}

/// 本地报价（点数，按最坏情况）。规则来自官方文档：
/// 搜索 basic 1 点、advanced 2 点；取页每 5 个成功的网址 1 点（advanced 2 点）；
/// 地图每 10 页 1 点（有 `instructions` 2 点）；爬站 = 地图 + 取页。不足 5 或 10 的按一组算。
pub mod quote {
    use super::{Crawl, Depth};

    pub fn search(depth: Depth) -> f64 {
        depth.factor()
    }

    pub fn extract(urls: usize, depth: Depth) -> f64 {
        urls.div_ceil(5) as f64 * depth.factor()
    }

    pub fn map(limit: u32, instructions: bool) -> f64 {
        limit.div_ceil(10) as f64 * if instructions { 2.0 } else { 1.0 }
    }

    pub fn crawl(c: &Crawl) -> f64 {
        map(c.limit, c.instructions.is_some()) + extract(c.limit as usize, c.extract_depth)
    }
}

/// Tavily 客户端：密钥由 App 交进来，不留、不打印。
pub struct Client<'a> {
    pub key: &'a str,
    /// 默认 [`BASE`]；测试、代理改它。
    pub base: &'a str,
    pub transport: &'a dyn Transport,
}

impl<'a> Client<'a> {
    pub fn new(key: &'a str, transport: &'a dyn Transport) -> Self {
        Self {
            key,
            base: BASE,
            transport,
        }
    }

    pub fn search(&self, s: &Search, cancel: &Cancel) -> Result<Reply<Vec<Hit>>, Error> {
        let mut body = json!({
            "query": s.query.trim(),
            "max_results": s.max_results.clamp(1, 20),
            "search_depth": s.depth.key(),
            "topic": if s.topic == "news" { "news" } else { "general" },
            "include_answer": false,
            "include_raw_content": false,
            "include_images": false,
        });
        if let Some(r) = &s.recency {
            body["time_range"] = json!(r);
        }
        if !s.domains.is_empty() {
            body["include_domains"] = json!(s.domains);
        }
        if !s.exclude_domains.is_empty() {
            body["exclude_domains"] = json!(s.exclude_domains);
        }
        let (v, credits, request_id) =
            self.post("search", body, Duration::from_secs(60), cancel)?;
        let hits = items(&v["results"])
            .filter_map(|r| {
                Some(Hit {
                    url: text(&r["url"])?,
                    title: text(&r["title"]).unwrap_or_default(),
                    content: text(&r["content"]).unwrap_or_default(),
                    published: text(&r["published_date"]),
                    score: r["score"].as_f64().unwrap_or(0.0),
                })
            })
            .collect();
        Ok(Reply {
            value: hits,
            credits,
            request_id,
        })
    }

    /// 取页（最多 20 个网址）。正文是 Markdown。
    pub fn extract(
        &self,
        urls: &[String],
        depth: Depth,
        cancel: &Cancel,
    ) -> Result<Reply<Extracted>, Error> {
        let body = json!({
            "urls": urls.iter().take(20).collect::<Vec<_>>(),
            "extract_depth": depth.key(),
            "format": "markdown",
            "include_images": false,
        });
        let (v, credits, request_id) =
            self.post("extract", body, Duration::from_secs(70), cancel)?;
        Ok(Reply {
            value: Extracted {
                pages: pages(&v["results"]),
                failed: items(&v["failed_results"])
                    .filter_map(|f| text(&f["url"]))
                    .collect(),
            },
            credits,
            request_id,
        })
    }

    /// 爬站：从 `url` 起往下爬，每页的正文（Markdown）。
    pub fn crawl(&self, c: &Crawl, cancel: &Cancel) -> Result<Reply<Vec<Page>>, Error> {
        let mut body = crawl_body(c);
        body["extract_depth"] = json!(c.extract_depth.key());
        body["format"] = json!("markdown");
        let (v, credits, request_id) =
            self.post("crawl", body, Duration::from_secs(100), cancel)?;
        Ok(Reply {
            value: pages(&v["results"]),
            credits,
            request_id,
        })
    }

    /// 站内地图：从 `url` 起找到的网址。
    pub fn map(&self, c: &Crawl, cancel: &Cancel) -> Result<Reply<Vec<String>>, Error> {
        let (v, credits, request_id) =
            self.post("map", crawl_body(c), Duration::from_secs(100), cancel)?;
        Ok(Reply {
            value: items(&v["results"]).filter_map(text).collect(),
            credits,
            request_id,
        })
    }

    /// 发一个请求，429、434、5xx 等了再试。交回回复、点数和请求编号。
    fn post(
        &self,
        endpoint: &str,
        mut body: Value,
        timeout: Duration,
        cancel: &Cancel,
    ) -> Result<(Value, Option<f64>, Option<String>), Error> {
        if self.key.trim().is_empty() {
            return Err(Error::Auth);
        }
        body["include_usage"] = json!(true);
        let url = format!("{}/{endpoint}", self.base.trim_end_matches('/'));
        let mut req = Request::post(url, serde_json::to_vec(&body).unwrap_or_default())
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.key.trim()));
        req.timeout = timeout;
        req.connect_timeout = Duration::from_secs(15);
        req.max_bytes = 20 * 1024 * 1024;
        let crawling = matches!(endpoint, "crawl" | "map");
        let mut attempt = 0;
        loop {
            cancel.check().map_err(|_| Error::Cancelled)?;
            let resp = self.transport.send(&req, cancel)?;
            match resp.status {
                200 => {
                    let v: Value =
                        serde_json::from_slice(&resp.body).map_err(|_| Error::Service)?;
                    let credits = v.pointer("/usage/credits").and_then(Value::as_f64);
                    let request_id = text(&v["request_id"]);
                    return Ok((v, credits, request_id));
                }
                429 | 434 | 500..=599 if attempt < WAITS.len() => {
                    let wait = resp
                        .header_value("retry-after")
                        .and_then(|s| s.trim().parse::<u64>().ok())
                        .map(|s| Duration::from_secs(s).min(MAX_WAIT))
                        .unwrap_or(WAITS[attempt]);
                    sleep(wait, cancel)?;
                    attempt += 1;
                }
                401 => return Err(Error::Auth),
                403 if crawling => return Err(Error::UnsupportedSite),
                403 => return Err(Error::Auth),
                400 | 422 => return Err(Error::BadRequest),
                432 | 433 => return Err(Error::Quota),
                429 | 434 => return Err(Error::RateLimited),
                _ => return Err(Error::Service),
            }
        }
    }
}

fn crawl_body(c: &Crawl) -> Value {
    let mut body = json!({
        "url": c.url.trim(),
        "max_depth": c.max_depth.clamp(1, 5),
        "limit": c.limit.max(1),
        "allow_external": false,
        "timeout": 90,
    });
    if let Some(i) = c
        .instructions
        .as_deref()
        .map(str::trim)
        .filter(|i| !i.is_empty())
    {
        body["instructions"] = json!(i);
    }
    if !c.select_paths.is_empty() {
        body["select_paths"] = json!(c.select_paths);
    }
    body
}

fn items(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}

fn text(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn pages(v: &Value) -> Vec<Page> {
    items(v)
        .filter_map(|r| {
            Some(Page {
                url: text(&r["url"])?,
                content: r["raw_content"].as_str().unwrap_or("").trim().to_string(),
            })
        })
        .collect()
}

/// 等一会儿，每 50 ms 查一次取消。
fn sleep(wait: Duration, cancel: &Cancel) -> Result<(), Error> {
    let until = Instant::now() + wait;
    while Instant::now() < until {
        cancel.check().map_err(|_| Error::Cancelled)?;
        std::thread::sleep(
            Duration::from_millis(50).min(until.saturating_duration_since(Instant::now())),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_follow_the_credit_rules() {
        assert_eq!(quote::search(Depth::Basic), 1.0);
        assert_eq!(quote::search(Depth::Advanced), 2.0);
        assert_eq!(quote::extract(1, Depth::Basic), 1.0, "不足 5 个按一组");
        assert_eq!(quote::extract(5, Depth::Basic), 1.0);
        assert_eq!(quote::extract(6, Depth::Advanced), 4.0);
        assert_eq!(quote::map(10, false), 1.0);
        assert_eq!(quote::map(25, true), 6.0);
        // 官方的例子：10 页 basic 爬站 3 点，advanced 5 点
        let c = Crawl {
            limit: 10,
            ..Default::default()
        };
        assert_eq!(quote::crawl(&c), 3.0);
        let c = Crawl {
            extract_depth: Depth::Advanced,
            ..c
        };
        assert_eq!(quote::crawl(&c), 5.0);
    }
}
