//! 默认的 `Ureq` 那条路：本机起一个 HTTP 服务（127.0.0.1），测跳转、gzip 解压、截断、超时、钉地址。不连外网。

use engine_web::engine_core::Cancel;
use engine_web::transport::{Request, Transport};
use engine_web::{Error, FetchOptions, Ureq, fetch};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// 「压缩过的正文，应该自动解开。」的 gzip。
const GZIP: &[u8] = &[
    31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 1, 42, 0, 213, 255, 229, 142, 139, 231, 188, 169, 232, 191,
    135, 231, 154, 132, 230, 173, 163, 230, 150, 135, 239, 188, 140, 229, 186, 148, 232, 175, 165,
    232, 135, 170, 229, 138, 168, 232, 167, 163, 229, 188, 128, 227, 128, 130, 103, 229, 226, 210,
    42, 0, 0, 0,
];

struct Server {
    addr: SocketAddr,
    hits: Arc<AtomicUsize>,
}

fn serve() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            counter.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || answer(stream));
        }
    });
    Server { addr, hits }
}

fn answer(mut stream: TcpStream) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).unwrap_or(0) == 0 {
            return;
        }
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head).to_string();
    let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
    let (status, headers, body): (&str, Vec<String>, Vec<u8>) = match path.as_str() {
        "/hello" => (
            "200 OK",
            vec!["Content-Type: text/html; charset=utf-8".into()],
            "<p>你好</p>".as_bytes().to_vec(),
        ),
        "/r" => ("302 Found", vec!["Location: /hello".into()], Vec::new()),
        "/gz" => (
            "200 OK",
            vec![
                "Content-Type: text/plain; charset=utf-8".into(),
                "Content-Encoding: gzip".into(),
            ],
            GZIP.to_vec(),
        ),
        "/big" => (
            "200 OK",
            vec!["Content-Type: text/plain".into()],
            vec![b'x'; 300_000],
        ),
        "/slow" => {
            std::thread::sleep(Duration::from_secs(3));
            ("200 OK", vec![], b"late".to_vec())
        }
        "/ua" => {
            let ua = head
                .lines()
                .find_map(|l| {
                    l.strip_prefix("user-agent: ")
                        .or(l.strip_prefix("User-Agent: "))
                })
                .unwrap_or("")
                .to_string();
            (
                "200 OK",
                vec!["Content-Type: text/plain".into()],
                ua.into_bytes(),
            )
        }
        _ => ("404 Not Found", vec![], b"no".to_vec()),
    };
    let mut out = format!(
        "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for h in headers {
        out += &h;
        out += "\r\n";
    }
    out += "\r\n";
    let _ = stream.write_all(out.as_bytes());
    let _ = stream.write_all(&body);
}

fn local() -> FetchOptions {
    FetchOptions {
        allow_private: true,
        ..Default::default()
    }
}

#[test]
fn ureq_follows_redirects_and_unzips() {
    let s = serve();
    let base = format!("http://{}", s.addr);
    let got = fetch(&format!("{base}/r"), &local(), &Ureq, &Cancel::new()).unwrap();
    assert_eq!(got.url, format!("{base}/hello"));
    assert_eq!(got.bytes, "<p>你好</p>".as_bytes());
    assert_eq!(
        got.content_type.as_deref(),
        Some("text/html; charset=utf-8")
    );
    let gz = fetch(&format!("{base}/gz"), &local(), &Ureq, &Cancel::new()).unwrap();
    assert_eq!(
        String::from_utf8(gz.bytes).unwrap(),
        "压缩过的正文，应该自动解开。"
    );
    let opts = FetchOptions {
        user_agent: "Pith/1.1.0".into(),
        ..local()
    };
    let ua = fetch(&format!("{base}/ua"), &opts, &Ureq, &Cancel::new()).unwrap();
    assert_eq!(ua.bytes, b"Pith/1.1.0", "App 给的 User-Agent");
}

#[test]
fn ureq_cuts_big_bodies_and_times_out() {
    let s = serve();
    let base = format!("http://{}", s.addr);
    let opts = FetchOptions {
        max_bytes: 100_000,
        ..local()
    };
    let big = fetch(&format!("{base}/big"), &opts, &Ureq, &Cancel::new()).unwrap();
    assert_eq!((big.bytes.len(), big.truncated), (100_000, true));
    let quick = FetchOptions {
        timeout: Duration::from_millis(800),
        ..local()
    };
    assert_eq!(
        fetch(&format!("{base}/slow"), &quick, &Ureq, &Cancel::new()).unwrap_err(),
        Error::Timeout
    );
}

#[test]
fn ureq_connects_only_to_the_pinned_addresses() {
    let s = serve();
    // 名字解析不了（.invalid），只靠钉住的地址连上
    let mut req = Request::get(format!("http://pinned.invalid:{}/hello", s.addr.port()));
    req.pinned = vec![s.addr];
    let resp = Ureq.send(&req, &Cancel::new()).unwrap();
    assert_eq!(
        (resp.status, resp.body.as_slice()),
        (200, "<p>你好</p>".as_bytes())
    );
    let unpinned = Request::get(format!("http://pinned.invalid:{}/hello", s.addr.port()));
    assert!(Ureq.send(&unpinned, &Cancel::new()).is_err());
}

#[test]
fn ureq_never_reaches_private_addresses_by_default() {
    let s = serve();
    let before = s.hits.load(Ordering::SeqCst);
    assert_eq!(
        fetch(
            &format!("http://{}/hello", s.addr),
            &FetchOptions::default(),
            &Ureq,
            &Cancel::new()
        )
        .unwrap_err(),
        Error::PrivateAddress
    );
    assert_eq!(
        fetch(
            &format!("http://localhost:{}/hello", s.addr.port()),
            &FetchOptions::default(),
            &Ureq,
            &Cancel::new()
        )
        .unwrap_err(),
        Error::PrivateAddress
    );
    assert_eq!(s.hits.load(Ordering::SeqCst), before, "一个连接都没有");
}
