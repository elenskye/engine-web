//! 发请求的那一层：发一个请求、不跟跳转（[`Transport::send`]），查主机的地址给内网检查用（[`Transport::resolve`]）。
//! 跳转、内网、同站、大小上限这些规矩都在 `fetch`，不在这里。
//!
//! 默认的 [`Ureq`]：Mac 上系统 TLS、别的平台 rustls；自动解压 gzip、br；`pinned` 不空时只连这几个检查过的地址
//! （防 DNS 重绑定：检查和连接用的是同一次解析）；收到 `max_bytes` 就停，标 `truncated`；读的时候逐块查取消。
//! 测试和 App 的模拟模式用 feature `fake` 的 `Fake`。

use crate::Error;
use engine_core::Cancel;
use engine_core::call::Zeroizing;
use std::fmt;
use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    Get,
    Post,
}

/// 一个请求。头的值可能是 Cookie、密钥，用 `Zeroizing` 收，`Debug` 不打出来。
#[derive(Clone)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, Zeroizing<String>)>,
    pub body: Option<Vec<u8>>,
    /// 只连这些地址（内网检查过的）；空就照常解析（走代理、Tavily 这类）。
    pub pinned: Vec<SocketAddr>,
    pub user_agent: String,
    pub proxy: Option<String>,
    pub timeout: Duration,
    pub connect_timeout: Duration,
    /// 最多收多少字节（解压后）；再多就停，`Response::truncated` 为真。
    pub max_bytes: u64,
}

impl Request {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            url: url.into(),
            headers: Vec::new(),
            body: None,
            pinned: Vec::new(),
            user_agent: format!("engine-web/{}", crate::VERSION),
            proxy: None,
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(15),
            max_bytes: 10 * 1024 * 1024,
        }
    }

    pub fn post(url: impl Into<String>, body: Vec<u8>) -> Self {
        Self {
            method: Method::Post,
            body: Some(body),
            ..Self::get(url)
        }
    }

    pub fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers
            .push((name.to_string(), Zeroizing::new(value.into())));
        self
    }

    /// 这个请求头的值（不分大小写）。
    pub fn header_value(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("url", &self.url)
            .field(
                "headers",
                &self.headers.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            )
            .field("pinned", &self.pinned)
            .field("max_bytes", &self.max_bytes)
            .finish_non_exhaustive()
    }
}

/// 一个回复。跳转（3xx）也原样交回，由 `fetch` 处理。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    /// 头的名字是小写。
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// 超过 `max_bytes`，只收了前面那部分。
    pub truncated: bool,
}

impl Response {
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            truncated: false,
        }
    }

    /// 带 `Content-Type` 的回复。
    pub fn with(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        Self::new(status)
            .header("content-type", content_type)
            .body(body)
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers
            .push((name.to_ascii_lowercase(), value.to_string()));
        self
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    /// 这个头的值（不分大小写）。
    pub fn header_value(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// 发请求的那一层。全部同步；实现要 `Send + Sync`，App 可以在线程间共用一份。
pub trait Transport: Send + Sync {
    /// 查主机的地址（内网检查用）。查不到交 [`Error::UnknownHost`]。
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, Error>;
    /// 发一个请求，不跟跳转。连不上、超时交 [`Error::Network`]、[`Error::Timeout`]；HTTP 状态码不算错，照实交回。
    fn send(&self, req: &Request, cancel: &Cancel) -> Result<Response, Error>;
}

/// 默认的 [`Transport`]：ureq。每个请求新建一个连接（要钉地址），不留连接池。
#[derive(Debug, Default, Clone, Copy)]
pub struct Ureq;

impl Transport for Ureq {
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, Error> {
        let addrs: Vec<IpAddr> = (host, port)
            .to_socket_addrs()
            .map_err(|_| Error::UnknownHost)?
            .map(|a| a.ip())
            .collect();
        if addrs.is_empty() {
            return Err(Error::UnknownHost);
        }
        Ok(addrs)
    }

    fn send(&self, req: &Request, cancel: &Cancel) -> Result<Response, Error> {
        cancel.check().map_err(|_| Error::Cancelled)?;
        let proxy = match req
            .proxy
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            Some(p) => Some(ureq::Proxy::new(p).map_err(|_| Error::Network)?),
            None => None,
        };
        let config = ureq::Agent::config_builder()
            .tls_config(tls())
            .proxy(proxy.clone())
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(req.timeout))
            .timeout_connect(Some(req.connect_timeout))
            .user_agent(req.user_agent.as_str())
            .max_idle_connections(0)
            .build();
        // 走代理时解析的是代理的地址，不能钉
        let agent = if req.pinned.is_empty() || proxy.is_some() {
            ureq::Agent::new_with_config(config)
        } else {
            ureq::Agent::with_parts(
                config,
                ureq::unversioned::transport::DefaultConnector::new(),
                Pinned(req.pinned.clone()),
            )
        };
        let result = match (req.method, &req.body) {
            (Method::Get, _) => {
                let mut b = agent.get(&req.url);
                for (name, value) in &req.headers {
                    b = b.header(name.as_str(), value.as_str());
                }
                b.call()
            }
            (Method::Post, body) => {
                let mut b = agent.post(&req.url);
                for (name, value) in &req.headers {
                    b = b.header(name.as_str(), value.as_str());
                }
                b.send(body.as_deref().unwrap_or_default())
            }
        };
        let mut resp = result.map_err(error_of)?;
        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .filter_map(|(n, v)| {
                v.to_str()
                    .ok()
                    .map(|v| (n.as_str().to_ascii_lowercase(), v.to_string()))
            })
            .collect();
        let mut reader = resp.body_mut().with_config().limit(u64::MAX).reader();
        let (body, truncated) = read_capped(&mut reader, req.max_bytes, cancel)?;
        Ok(Response {
            status,
            headers,
            body,
            truncated,
        })
    }
}

/// 读到 `max` 字节为止（多出来的不收，标 `truncated`），每块查一次取消。
fn read_capped(
    reader: &mut impl Read,
    max: u64,
    cancel: &Cancel,
) -> Result<(Vec<u8>, bool), Error> {
    let mut body = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        cancel.check().map_err(|_| Error::Cancelled)?;
        let n = match reader.read(&mut buf) {
            Ok(0) => return Ok((body, false)),
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => return Err(Error::Timeout),
            // 收了一部分就断了：有字就当不全交回，一个字都没有算连不上
            Err(_) if !body.is_empty() => return Ok((body, true)),
            Err(_) => return Err(Error::Network),
        };
        let room = max.saturating_sub(body.len() as u64) as usize;
        if n > room {
            body.extend_from_slice(&buf[..room]);
            return Ok((body, true));
        }
        body.extend_from_slice(&buf[..n]);
    }
}

fn error_of(e: ureq::Error) -> Error {
    match e {
        ureq::Error::Timeout(_) => Error::Timeout,
        ureq::Error::HostNotFound => Error::UnknownHost,
        ureq::Error::BadUri(_) | ureq::Error::Http(_) => Error::BadUrl,
        ureq::Error::Io(io) if io.kind() == std::io::ErrorKind::TimedOut => Error::Timeout,
        _ => Error::Network,
    }
}

#[cfg(target_os = "macos")]
fn tls() -> ureq::tls::TlsConfig {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build()
}

#[cfg(not(target_os = "macos"))]
fn tls() -> ureq::tls::TlsConfig {
    ureq::tls::TlsConfig::default()
}

/// 只交回检查过的那几个地址的解析器。
#[derive(Debug)]
struct Pinned(Vec<SocketAddr>);

impl ureq::unversioned::resolver::Resolver for Pinned {
    fn resolve(
        &self,
        _uri: &ureq::http::Uri,
        _config: &ureq::config::Config,
        _timeout: ureq::unversioned::transport::NextTimeout,
    ) -> Result<ureq::unversioned::resolver::ResolvedSocketAddrs, ureq::Error> {
        let mut out = self.empty();
        for addr in self.0.iter().take(16) {
            out.push(*addr);
        }
        if out.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_reads_stop_at_the_limit() {
        let cancel = Cancel::new();
        let data = vec![7u8; 200_000];
        let (body, cut) = read_capped(&mut data.as_slice(), 150_000, &cancel).unwrap();
        assert_eq!((body.len(), cut), (150_000, true));
        let (body, cut) = read_capped(&mut data.as_slice(), 200_000, &cancel).unwrap();
        assert_eq!((body.len(), cut), (200_000, false), "正好等于上限不算截断");
        cancel.cancel();
        assert_eq!(
            read_capped(&mut data.as_slice(), 10, &cancel).unwrap_err(),
            Error::Cancelled
        );
    }

    #[test]
    fn request_debug_hides_header_values() {
        let req = Request::get("https://x.example/").header("Cookie", "session=secret");
        let shown = format!("{req:?}");
        assert!(
            shown.contains("Cookie") && !shown.contains("secret"),
            "{shown}"
        );
        assert_eq!(req.header_value("cookie"), Some("session=secret"));
    }
}
