//! 模拟服务（feature `fake`）：按网址回写好的回复，记下每个请求。测试和 App 的模拟模式用它，不连外网。
//!
//! ```ignore
//! let fake = Fake::new()
//!     .page("https://news.example.com/a", "text/html; charset=utf-8", "<p>…</p>")
//!     .redirect("https://news.example.com/old", "/a")
//!     .host("intranet.example.com", "10.0.0.5")
//!     .reply(Method::Post, "https://api.tavily.com/search", Response::with(200, "application/json", r#"{"results":[]}"#));
//! ```

use crate::Error;
use crate::transport::{Method, Request, Response, Transport};
use engine_core::Cancel;
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;

/// 记下的一个请求（模拟服务里头的值照原样记，测试要核对）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub pinned: Vec<SocketAddr>,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// 请求体当 JSON 读。
    pub fn json(&self) -> serde_json::Value {
        self.body
            .as_deref()
            .and_then(|b| serde_json::from_slice(b).ok())
            .unwrap_or_default()
    }
}

/// 一个网址写好的几次回复。
type Replies = VecDeque<Result<Response, Error>>;

#[derive(Default)]
pub struct Fake {
    /// 同一个网址的几次回复按顺序用，最后一个一直重复。
    routes: Mutex<HashMap<(Method, String), Replies>>,
    hosts: HashMap<String, Vec<IpAddr>>,
    seen: Mutex<Vec<Seen>>,
}

impl Fake {
    pub fn new() -> Self {
        Self::default()
    }

    /// 一个 200 的页面。
    pub fn page(self, url: &str, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        self.reply(Method::Get, url, Response::with(200, content_type, body))
    }

    /// 一个 302 跳转。
    pub fn redirect(self, from: &str, to: &str) -> Self {
        self.reply(Method::Get, from, Response::new(302).header("location", to))
    }

    /// 写好的回复。同一个网址写几次就按顺序回（重试的测试用），最后一个一直重复。
    pub fn reply(self, method: Method, url: &str, resp: Response) -> Self {
        self.push(method, url, Ok(resp))
    }

    /// 这个网址连不上、超时这类。
    pub fn fail(self, method: Method, url: &str, error: Error) -> Self {
        self.push(method, url, Err(error))
    }

    fn push(self, method: Method, url: &str, resp: Result<Response, Error>) -> Self {
        self.routes
            .lock()
            .unwrap()
            .entry((method, url.to_string()))
            .or_default()
            .push_back(resp);
        self
    }

    /// 主机解析成这个地址（内网检查的测试用）。没写的主机解析成一个公网地址；写成空串表示解析不了。
    pub fn host(mut self, host: &str, ip: &str) -> Self {
        let ips = ip.parse().into_iter().collect();
        self.hosts.insert(host.to_ascii_lowercase(), ips);
        self
    }

    /// 到现在为止收到的请求。
    pub fn requests(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

impl Transport for Fake {
    fn simulated(&self) -> bool {
        true
    }

    fn resolve(&self, host: &str, _port: u16) -> Result<Vec<IpAddr>, Error> {
        match self.hosts.get(&host.to_ascii_lowercase()) {
            Some(ips) if ips.is_empty() => Err(Error::UnknownHost),
            Some(ips) => Ok(ips.clone()),
            None => Ok(vec!["93.184.216.34".parse().unwrap()]),
        }
    }

    fn send(&self, req: &Request, cancel: &Cancel) -> Result<Response, Error> {
        cancel.check().map_err(|_| Error::Cancelled)?;
        self.seen.lock().unwrap().push(Seen {
            method: req.method,
            url: req.url.clone(),
            headers: req
                .headers
                .iter()
                .map(|(n, v)| (n.clone(), v.to_string()))
                .collect(),
            body: req.body.clone(),
            pinned: req.pinned.clone(),
        });
        let mut routes = self.routes.lock().unwrap();
        let reply = match routes.get_mut(&(req.method, req.url.clone())) {
            Some(queue) if queue.len() > 1 => queue.pop_front().unwrap(),
            Some(queue) => queue.front().cloned().unwrap(),
            None => Ok(Response::with(404, "text/html", "<h1>404</h1>")),
        };
        let mut resp = reply?;
        if resp.body.len() as u64 > req.max_bytes {
            resp.body.truncate(req.max_bytes as usize);
            resp.truncated = true;
        }
        Ok(resp)
    }
}
