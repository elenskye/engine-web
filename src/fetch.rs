//! 抓取：网址 → 字节。规矩见 `docs/design.md` §3，从 Peck 的 `web.rs` 和 Pith 的 `download.rs` 搬来。
//!
//! - 网址只认 http(s)，不带用户名密码，要有主机。
//! - 默认不抓本机和内网：名字是本机的（`localhost`、`*.local`……）、IP 是内网的、解析出来有一个内网地址的，都拒；
//!   检查过的地址钉住再连（`Request::pinned`）。App 可以放开（`allow_private`）。
//! - 自己跟跳转，每一跳重新查；默认只跟同一个网站（只差 `www.`、`http` → `https`），跳到别的网站交回新网址；
//!   App 放开时，跳到别的网站不带 App 给的 Cookie 和 Authorization。
//! - 超时、大小上限 App 可以改，但不超过引擎的上限（[`LIMITS`]）。超了大小按收到的处理，标 `truncated`。

use crate::Error;
use crate::transport::{Request, Transport};
use engine_core::Cancel;
use engine_core::call::Zeroizing;
use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use url::Url;

/// App 能改到的上限。
pub struct Limits {
    pub timeout: Duration,
    pub max_bytes: u64,
    pub max_redirects: u32,
}

/// 引擎的上限：超时 120 s、50 MB、跳转 10 次。
pub const LIMITS: Limits = Limits {
    timeout: Duration::from_secs(120),
    max_bytes: 50 * 1024 * 1024,
    max_redirects: 10,
};

/// 抓取的设置。App 交进来；`Default` 是引擎的默认值。
#[derive(Clone)]
pub struct FetchOptions {
    /// 整个请求的超时（每一跳各算），默认 30 s。
    pub timeout: Duration,
    /// 连接的超时，默认 15 s。
    pub connect_timeout: Duration,
    /// 最多收多少字节（解压后），默认 10 MB。
    pub max_bytes: u64,
    /// 最多跟几次跳转，默认 5。
    pub max_redirects: u32,
    /// 跳到别的网站也跟（默认不跟，交 [`Error::RedirectElsewhere`]）。用户自己给的网址可以放开。
    pub follow_other_sites: bool,
    /// 本机和内网地址也抓（默认不抓）。
    pub allow_private: bool,
    /// App 给；默认 `engine-web/版本`。
    pub user_agent: String,
    /// App 给的请求头（Cookie、Accept-Language……）。值不进日志、错误信息。同名的覆盖引擎的默认值（`Accept`）。
    pub headers: Vec<(String, Zeroizing<String>)>,
    /// 代理（`http://…`、`socks5://…`）。走代理时只按名字和 IP 查内网，不解析。
    pub proxy: Option<String>,
    /// 同一站点两次请求的间隔（App 持有，几个抓取共用一份）。
    pub pacer: Option<Arc<HostPacer>>,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(15),
            max_bytes: 10 * 1024 * 1024,
            max_redirects: 5,
            follow_other_sites: false,
            allow_private: false,
            user_agent: format!("engine-web/{}", crate::VERSION),
            headers: Vec::new(),
            proxy: None,
            pacer: None,
        }
    }
}

impl fmt::Debug for FetchOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FetchOptions")
            .field("timeout", &self.timeout)
            .field("max_bytes", &self.max_bytes)
            .field("max_redirects", &self.max_redirects)
            .field("follow_other_sites", &self.follow_other_sites)
            .field("allow_private", &self.allow_private)
            .field(
                "headers",
                &self.headers.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

/// 默认的 `Accept`：先要网页和 Markdown（Peck 的写法）。
const ACCEPT: &str =
    "text/html,application/xhtml+xml,text/markdown;q=0.9,text/plain;q=0.8,*/*;q=0.5";

/// 抓到的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// 最后的网址（跟过跳转之后）。
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
    /// 超过大小上限，只收了前面那部分。
    pub truncated: bool,
    /// 跳过的网址（按顺序，不含最后那个）。
    pub redirects: Vec<String>,
}

/// 抓一个网址：自己跟跳转，每一跳查网址和地址。2xx 交回字节；别的状态码交 [`Error::HttpStatus`]。
pub fn fetch(
    url: &str,
    opts: &FetchOptions,
    transport: &dyn Transport,
    cancel: &Cancel,
) -> Result<Fetched, Error> {
    let max_redirects = opts.max_redirects.min(LIMITS.max_redirects);
    let mut current = check_url(url)?;
    let start_host = host_of(&current);
    let mut redirects = Vec::new();
    for hop in 0..=max_redirects {
        cancel.check().map_err(|_| Error::Cancelled)?;
        let pinned = if opts.allow_private {
            Vec::new()
        } else {
            check_host(&current, opts.proxy.is_some(), transport)?
        };
        if let Some(pacer) = &opts.pacer {
            pacer.wait(&host_of(&current), cancel)?;
        }
        let mut req = Request::get(current.as_str());
        req.pinned = pinned;
        req.user_agent = opts.user_agent.clone();
        req.proxy = opts.proxy.clone();
        req.timeout = opts.timeout.min(LIMITS.timeout);
        req.connect_timeout = opts.connect_timeout.min(req.timeout);
        req.max_bytes = opts.max_bytes.min(LIMITS.max_bytes);
        let elsewhere = host_of(&current) != start_host;
        if !opts
            .headers
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case("accept"))
        {
            req = req.header("Accept", ACCEPT);
        }
        for (name, value) in &opts.headers {
            // 跟到别的网站：App 的 Cookie、Authorization 不带过去
            if elsewhere
                && (name.eq_ignore_ascii_case("cookie")
                    || name.eq_ignore_ascii_case("authorization"))
            {
                continue;
            }
            req.headers.push((name.clone(), value.clone()));
        }
        let resp = transport.send(&req, cancel)?;
        if matches!(resp.status, 301 | 302 | 303 | 307 | 308) {
            let next = resp
                .header_value("location")
                .and_then(|l| current.join(l.trim()).ok())
                .ok_or(Error::HttpStatus {
                    status: resp.status,
                })?;
            if current.scheme() == "https"
                && next.scheme() == "http"
                && host_of(&current) == host_of(&next)
            {
                // 同一个网站降成 http 也不跟（Peck 的规矩），说明里讲清是降成了 http
                return Err(Error::InsecureRedirect {
                    to: next.to_string(),
                });
            }
            if !opts.follow_other_sites && !same_site(&current, &next) {
                return Err(Error::RedirectElsewhere {
                    to: next.to_string(),
                });
            }
            if hop == max_redirects {
                return Err(Error::TooManyRedirects);
            }
            redirects.push(current.to_string());
            current = check_url(next.as_str())?;
            continue;
        }
        if !(200..300).contains(&resp.status) {
            return Err(Error::HttpStatus {
                status: resp.status,
            });
        }
        return Ok(Fetched {
            url: current.to_string(),
            status: resp.status,
            content_type: resp.header_value("content-type").map(str::to_string),
            truncated: resp.truncated,
            bytes: resp.body,
            redirects,
        });
    }
    Err(Error::TooManyRedirects)
}

/// 网址能不能抓：http(s)、不带用户名密码、有主机。
pub fn check_url(raw: &str) -> Result<Url, Error> {
    let url = Url::parse(raw.trim()).map_err(|_| Error::BadUrl)?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none_or(str::is_empty)
    {
        return Err(Error::BadUrl);
    }
    Ok(url)
}

/// 主机能不能抓：名字是本机的拒；IP 是内网的拒；解析出来有一个内网地址就拒。交回要钉住的地址
/// （走代理时不解析，交回空的，由代理去连）。
fn check_host(
    url: &Url,
    proxied: bool,
    transport: &dyn Transport,
) -> Result<Vec<SocketAddr>, Error> {
    let host = url
        .host_str()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let port = url.port_or_known_default().unwrap_or(443);
    if local_name(&host) {
        return Err(Error::PrivateAddress);
    }
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = bare.parse::<IpAddr>() {
        return if is_public(ip) {
            Ok(vec![SocketAddr::new(ip, port)])
        } else {
            Err(Error::PrivateAddress)
        };
    }
    if proxied {
        return Ok(Vec::new());
    }
    let addrs = transport.resolve(bare, port)?;
    if addrs.is_empty() {
        return Err(Error::UnknownHost);
    }
    if addrs.iter().any(|ip| !is_public(*ip)) {
        return Err(Error::PrivateAddress);
    }
    Ok(addrs
        .into_iter()
        .map(|ip| SocketAddr::new(ip, port))
        .collect())
}

/// 名字就是本机、内网的（小写、不带结尾的点）。
pub fn local_name(host: &str) -> bool {
    host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
}

/// 公网地址：不是回环、私有网段、链路本地、100.64/10（运营商 NAT）、未指定、广播、组播、192.0.0/24、
/// IPv6 的 ULA / 链路本地；IPv4 映射的 IPv6 照 IPv4 看（Peck 的 `is_public`）。
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || o[0] == 0
                || (o[0] == 100 && (64..128).contains(&o[1]))
                || (o[0] == 192 && o[1] == 0 && o[2] == 0))
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public(IpAddr::V4(v4));
            }
            let seg = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (seg[0] & 0xfe00) == 0xfc00
                || (seg[0] & 0xffc0) == 0xfe80)
        }
    }
}

/// 同一个网站：主机一样（只差 `www.`），协议一样或者 http → https（Peck 的 `same_site`）。
pub fn same_site(from: &Url, to: &Url) -> bool {
    let scheme_ok =
        from.scheme() == to.scheme() || (from.scheme() == "http" && to.scheme() == "https");
    scheme_ok && host_of(from) == host_of(to)
}

/// 去掉 `www.` 的小写主机名。
fn host_of(url: &Url) -> String {
    url.host_str()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_ascii_lowercase()
        .trim_start_matches("www.")
        .to_string()
}

/// 同一站点两次请求之间至少隔多久（Pith 隔 1.5 s）。App 持有一份、几次抓取共用；要不要用由 App 定。
pub struct HostPacer {
    gap: Duration,
    next: Mutex<HashMap<String, Instant>>,
}

impl HostPacer {
    pub fn new(gap: Duration) -> Self {
        Self {
            gap,
            next: Mutex::new(HashMap::new()),
        }
    }

    /// 等到可以请求这个站点为止；等的时候每 50 ms 查一次取消。
    pub fn wait(&self, host: &str, cancel: &Cancel) -> Result<(), Error> {
        let until = {
            let mut next = self.next.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            let at = next.get(host).copied().filter(|t| *t > now).unwrap_or(now);
            next.insert(host.to_string(), at + self.gap);
            at
        };
        while Instant::now() < until {
            cancel.check().map_err(|_| Error::Cancelled)?;
            std::thread::sleep(
                Duration::from_millis(50).min(until.saturating_duration_since(Instant::now())),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_urls_without_credentials_are_fetched() {
        assert_eq!(check_url("ftp://example.com/a"), Err(Error::BadUrl));
        assert_eq!(check_url("file:///etc/hosts"), Err(Error::BadUrl));
        assert_eq!(
            check_url("https://user:pw@example.com/"),
            Err(Error::BadUrl)
        );
        assert_eq!(check_url("news/line5.html"), Err(Error::BadUrl));
        assert!(check_url(" https://example.com/docs ").is_ok());
    }

    #[test]
    fn private_addresses_are_not_public() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.9",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "224.0.0.1",
            "192.0.0.8",
            "::1",
            "::",
            "fd00::1",
            "fe80::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
        ] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["93.184.216.34", "2606:4700::1111", "1.1.1.1", "100.128.0.1"] {
            assert!(is_public(ip.parse().unwrap()), "{ip}");
        }
        for host in ["localhost", "dev.localhost", "printer.local", "db.internal"] {
            assert!(local_name(host), "{host}");
        }
        assert!(!local_name("localhost.example.com"));
    }

    #[test]
    fn same_site_allows_www_and_https_only() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert!(same_site(
            &u("http://example.com/a"),
            &u("https://www.example.com/b")
        ));
        assert!(same_site(
            &u("https://www.example.com/a"),
            &u("https://example.com/a/")
        ));
        assert!(
            !same_site(&u("https://example.com/a"), &u("http://example.com/a")),
            "https → http 不跟"
        );
        assert!(!same_site(
            &u("https://example.com/a"),
            &u("https://evil.example/a")
        ));
        assert!(!same_site(
            &u("https://a.example.com/"),
            &u("https://b.example.com/")
        ));
    }

    #[test]
    fn pacer_spaces_requests_to_the_same_site() {
        let pacer = HostPacer::new(Duration::from_millis(120));
        let cancel = Cancel::new();
        let start = Instant::now();
        pacer.wait("example.com", &cancel).unwrap();
        pacer.wait("other.example", &cancel).unwrap();
        assert!(start.elapsed() < Duration::from_millis(60), "不同站点不等");
        pacer.wait("example.com", &cancel).unwrap();
        assert!(start.elapsed() >= Duration::from_millis(110));
        cancel.cancel();
        assert_eq!(
            pacer.wait("example.com", &cancel).unwrap_err(),
            Error::Cancelled
        );
    }
}
