//! 编码识别：BOM → 合法的 UTF-8 → 声明的编码（HTTP 头 → 网页 meta / XML 声明）→ chardetng 探测。
//! 选型见 `docs/decisions/0005-encoding-detection.md`。
//!
//! 合法的 UTF-8 排在声明前面（和 Pith 一样）：老站改成 UTF-8 后常留着 `charset=gb2312`，按声明解反而乱码；
//! 而真正的 GBK、Big5 字节几乎不可能恰好是合法的 UTF-8。

use encoding_rs::{Encoding, UTF_8};

/// 解码成文字，交回文字和用的编码名（encoding_rs 的名字，如 `UTF-8`、`GBK`、`Big5`、`Shift_JIS`）。
pub fn decode(bytes: &[u8], content_type: Option<&str>) -> (String, &'static str) {
    decode_with(bytes, content_type, true, None)
}

/// `scan_page`：在开头 4 KB 里找网页自己声明的编码（HTML、XML 才找）。`tld`：网址的顶级域名，给探测当提示。
pub(crate) fn decode_with(
    bytes: &[u8],
    content_type: Option<&str>,
    scan_page: bool,
    tld: Option<&str>,
) -> (String, &'static str) {
    if let Some((enc, bom)) = Encoding::for_bom(bytes) {
        return (
            enc.decode_without_bom_handling(&bytes[bom..])
                .0
                .into_owned(),
            enc.name(),
        );
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return (s.to_string(), UTF_8.name());
    }
    let declared = content_type.and_then(charset_in).or_else(|| {
        if scan_page {
            declared_in_page(bytes)
        } else {
            None
        }
    });
    let enc = match declared {
        // 声明 UTF-8 而有坏字节（比如只收了前一部分、断在字中间）：还按 UTF-8，坏的替换掉
        Some(enc) => enc,
        // 没声明：只有结尾断了半个字，也还是 UTF-8
        None if truncated_utf8(bytes) => UTF_8,
        None => detect(bytes, tld),
    };
    (
        enc.decode_without_bom_handling(bytes).0.into_owned(),
        enc.name(),
    )
}

/// 没声明编码又不是 UTF-8：chardetng（Firefox 用的那个）按字节统计猜，顶级域名当提示。
fn detect(bytes: &[u8], tld: Option<&str>) -> &'static Encoding {
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    detector.feed(bytes, true);
    let tld = tld
        .map(str::to_ascii_lowercase)
        .filter(|t| !t.is_empty() && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
    detector.guess(
        tld.as_deref().map(str::as_bytes),
        chardetng::Utf8Detection::Deny,
    )
}

/// `…; charset=gbk` 里的编码。UTF-16 这类按 HTML 的规矩当 UTF-8。
fn charset_in(s: &str) -> Option<&'static Encoding> {
    let lower = s.to_ascii_lowercase();
    lower
        .match_indices("charset")
        .find_map(|(at, _)| label_after(&lower[at + 7..]))
}

/// 网页开头 4 KB 里声明的编码：`<meta charset>`、`<meta http-equiv content="…charset=…">`、`<?xml … encoding="…"?>`。
fn declared_in_page(bytes: &[u8]) -> Option<&'static Encoding> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).to_ascii_lowercase();
    if let Some(decl) = head.trim_start().strip_prefix("<?xml") {
        let decl = &decl[..decl.find("?>").unwrap_or(decl.len())];
        if let Some(at) = decl.find("encoding") {
            if let Some(enc) = label_after(&decl[at + 8..]) {
                return Some(enc);
            }
        }
    }
    charset_in(&head)
}

/// `= "gbk"` 这样的一截里的编码。
fn label_after(rest: &str) -> Option<&'static Encoding> {
    let rest = rest
        .trim_start()
        .strip_prefix('=')?
        .trim_start()
        .trim_start_matches(['"', '\'']);
    let label: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
        .collect();
    Encoding::for_label(label.as_bytes()).map(Encoding::output_encoding)
}

/// 合法的 UTF-8，只是结尾断了半个字（最多 3 个字节）。
fn truncated_utf8(bytes: &[u8]) -> bool {
    match std::str::from_utf8(bytes) {
        Ok(_) => true,
        Err(e) => {
            e.error_len().is_none() && bytes.len() - e.valid_up_to() <= 3 && e.valid_up_to() > 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_is_bom_utf8_declared_fallback() {
        let (gbk, _, _) = encoding_rs::GBK.encode("示例市统计局");
        assert_eq!(decode(&gbk, None), ("示例市统计局".to_string(), "GBK"));
        assert_eq!(
            decode(&gbk, Some("text/html; charset=GBK")),
            ("示例市统计局".to_string(), "GBK")
        );
        assert_eq!(decode(&gbk, Some("text/html; charset=\"gb2312\"")).1, "GBK");
        // 合法的 UTF-8 不管声明
        assert_eq!(
            decode("示例".as_bytes(), Some("text/html; charset=gb2312")),
            ("示例".to_string(), "UTF-8")
        );
        // BOM 最先
        assert_eq!(
            decode(b"\xEF\xBB\xBFabc", Some("text/html; charset=gbk")),
            ("abc".to_string(), "UTF-8")
        );
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("你好".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(decode(&utf16, None), ("你好".to_string(), "UTF-16LE"));
    }

    #[test]
    fn page_declarations() {
        let (big5, _, _) = encoding_rs::BIG5.encode("範例");
        let mut page = b"<html><head><meta http-equiv=\"Content-Type\" content=\"text/html; charset=big5\"></head><body>".to_vec();
        page.extend_from_slice(&big5);
        assert!(decode(&page, Some("text/html")).0.ends_with("範例"));
        let mut xml = b"<?xml version=\"1.0\" encoding=\"Shift_JIS\"?><t>".to_vec();
        xml.extend_from_slice(&encoding_rs::SHIFT_JIS.encode("日本語").0);
        assert_eq!(
            decode(&xml, None),
            (
                "<?xml version=\"1.0\" encoding=\"Shift_JIS\"?><t>日本語".to_string(),
                "Shift_JIS"
            )
        );
        // 第一个「charset」后面不是 = 的，接着往下找
        let mut tricky = b"<!-- charset notes --><meta charset=gbk>".to_vec();
        tricky.extend_from_slice(&encoding_rs::GBK.encode("中文").0);
        assert_eq!(decode(&tricky, None).1, "GBK");
        // 只在 HTML、XML 里找：别的类型里写着 charset 的字不算声明
        let mut json = br#"{"note": "charset=big5", "text": ""#.to_vec();
        json.extend_from_slice(&encoding_rs::GBK.encode("示例市统计局发布的数据").0);
        assert_eq!(decode(&json, None).1, "Big5");
        assert_eq!(decode_with(&json, None, false, None).1, "GBK");
        // 声明成 UTF-16 的按 UTF-8
        assert_eq!(decode(b"<meta charset=utf-16>\xff", None).1, "UTF-8");
    }

    #[test]
    fn undeclared_legacy_encodings_are_detected() {
        let cases: [(&'static Encoding, &str, Option<&str>); 4] = [
            (
                encoding_rs::BIG5,
                "本館秋季特展即日起至明年一月底展出，集結四十位藝術家的作品。",
                Some("tw"),
            ),
            (
                encoding_rs::SHIFT_JIS,
                "十二月一日から、図書館の開館時間は午前九時から午後五時半までとなります。",
                None,
            ),
            (
                encoding_rs::WINDOWS_1252,
                "À partir du 1er décembre, la bibliothèque sera ouverte du mardi au dimanche.",
                Some("fr"),
            ),
            (
                encoding_rs::GBK,
                "明天上午九点开会，请准时参加。",
                Some("com"),
            ),
        ];
        for (enc, text, tld) in cases {
            let (bytes, _, _) = enc.encode(text);
            assert_eq!(
                decode_with(&bytes, None, true, tld),
                (text.to_string(), enc.name())
            );
        }
    }

    #[test]
    fn broken_utf8_stays_utf8() {
        let mut cut = "正文".as_bytes().to_vec();
        cut.extend_from_slice(&"字".as_bytes()[..2]);
        assert_eq!(
            decode(&cut, None),
            ("正文\u{FFFD}".to_string(), "UTF-8"),
            "断在字中间"
        );
        let mut stray = "坏了一个字节".as_bytes().to_vec();
        stray.insert(3, 0xFF);
        assert_eq!(
            decode(&stray, Some("text/html; charset=utf-8")).1,
            "UTF-8",
            "声明了 UTF-8 就按 UTF-8"
        );
    }
}
