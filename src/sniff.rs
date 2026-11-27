//! 类型判断（从 Pith 的 `sniff` 搬来）：按内容判断，不信 Content-Type 和扩展名，看不出来才参考。

use crate::{Binary, Kind};

/// 判断字节是什么。读得了的交 [`Kind`]，不是网页的交 [`Binary`]（App 拿它交给别的引擎）。
///
/// `content_type`、`url`（看扩展名）只在内容看不出来时参考。空字节当作空的纯文本。
pub fn sniff(bytes: &[u8], content_type: Option<&str>, url: &str) -> Result<Kind, Binary> {
    let ct = content_type
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let ext = ext_of(url);
    if let Some(binary) = magic(bytes) {
        return Err(binary);
    }
    // UTF-16 的 BOM：字节里全是 0，跳过下面的二进制检查
    let utf16 = bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]);
    let head = &bytes[..bytes.len().min(2048)];
    if !utf16 {
        // 剩下的应当是文字：控制字符太多的不要
        let control = head
            .iter()
            .filter(|b| **b < 0x09 || (**b > 0x0D && **b < 0x20 && **b != 0x1B))
            .count();
        if control * 20 > head.len() {
            return Err(by_type(&ct, &ext).unwrap_or(Binary::Other));
        }
    }
    let start = lower_text(head, utf16);
    let start = start.trim_start_matches('\u{feff}').trim_start();
    // 开头就看得出来的
    if start.starts_with("<!doctype html") || start.starts_with("<html") {
        return Ok(Kind::Html);
    }
    if start.starts_with("<?xml")
        || start.starts_with("<rss")
        || start.starts_with("<feed")
        || start.starts_with("<svg")
    {
        return if start.contains("<!doctype html") || start.contains("<html") {
            Ok(Kind::Html)
        } else if start.contains("<svg") {
            Err(Binary::Image)
        } else {
            Ok(Kind::Xml)
        };
    }
    if (start.starts_with('{') || start.starts_with('['))
        && (ct.contains("json") || ext == "json" || looks_json(bytes))
    {
        return Ok(Kind::Json);
    }
    // 说是 Markdown 的（扩展名或 Content-Type）：里面夹着 HTML 标签也还是 Markdown（GitHub 的 README 常以 `<div align="center">` 开头）
    if ct.contains("markdown") || ext == "md" || ext == "markdown" {
        return Ok(Kind::Markdown);
    }
    let looks_html = [
        "<!doctype html",
        "<html",
        "<head",
        "<body",
        "<meta ",
        "<div",
        "<title",
    ]
    .iter()
    .any(|t| start.contains(t));
    if looks_html || ct.contains("html") {
        return Ok(Kind::Html);
    }
    // 像文字但说是二进制类型的（比如说是 image/png）：信它
    if let Some(binary) = binary_type(&ct) {
        return Err(binary);
    }
    if ct.contains("json") || ext == "json" {
        return Ok(Kind::Json);
    }
    if ct.ends_with("/xml") || ct.ends_with("+xml") || ext == "xml" {
        return Ok(Kind::Xml);
    }
    Ok(Kind::Text)
}

/// PDF 的文件头：在开头，或者前 1 KB 里、前面只有空白或二进制的垃圾字节（阅读器也这样容忍）。
/// 前面是文字的不算：讲 PDF 格式的网页标题里写着「%PDF-1.7」。
fn pdf_header(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let Some(at) = head.windows(5).position(|w| w == b"%PDF-") else {
        return false;
    };
    let before = &head[..at];
    before.iter().all(u8::is_ascii_whitespace)
        || std::str::from_utf8(before).is_err()
        || before
            .iter()
            .any(|b| *b < 0x09 || (0x0E..0x20).contains(b) || *b == 0x7F)
}

/// 开头的魔数。
fn magic(bytes: &[u8]) -> Option<Binary> {
    if pdf_header(bytes) {
        return Some(Binary::Pdf);
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        return Some(if has(bytes, b"word/document.xml") {
            Binary::Word
        } else if has(bytes, b"xl/workbook") {
            Binary::Excel
        } else if has(bytes, b"ppt/presentation.xml") {
            Binary::PowerPoint
        } else if bytes.len() > 58
            && &bytes[30..38] == b"mimetype"
            && bytes[38..].starts_with(b"application/epub+zip")
        {
            Binary::Epub
        } else {
            Binary::Archive
        });
    }
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        // 旧版 Office（OLE 复合文档）：目录里的流名是 UTF-16LE
        let utf16 = |s: &str| {
            s.encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<u8>>()
        };
        return Some(if has(bytes, &utf16("WordDocument")) {
            Binary::Word
        } else if has(bytes, &utf16("Workbook")) || has(bytes, &utf16("Book")) {
            Binary::Excel
        } else if has(bytes, &utf16("PowerPoint Document")) {
            Binary::PowerPoint
        } else {
            Binary::Other
        });
    }
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        return Some(match &bytes[8..12] {
            b"avif" | b"avis" | b"heic" | b"heix" | b"mif1" | b"msf1" => Binary::Image,
            b"M4A " | b"M4B " => Binary::Audio,
            _ => Binary::Video,
        });
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") {
        return Some(match &bytes[8..12] {
            b"WEBP" => Binary::Image,
            b"WAVE" => Binary::Audio,
            b"AVI " => Binary::Video,
            _ => Binary::Other,
        });
    }
    const TABLE: &[(&[u8], Binary)] = &[
        (b"\x89PNG", Binary::Image),
        (b"\xFF\xD8\xFF", Binary::Image),
        (b"GIF87a", Binary::Image),
        (b"GIF89a", Binary::Image),
        (b"II*\x00", Binary::Image),
        (b"MM\x00*", Binary::Image),
        (b"\x00\x00\x01\x00", Binary::Image),
        (b"\x1F\x8B", Binary::Archive),
        (b"7z\xBC\xAF\x27\x1C", Binary::Archive),
        (b"Rar!\x1A\x07", Binary::Archive),
        (b"BZh", Binary::Archive),
        (b"\xFD7zXZ\x00", Binary::Archive),
        (b"ID3", Binary::Audio),
        (b"OggS", Binary::Audio),
        (b"fLaC", Binary::Audio),
        (b"\x1A\x45\xDF\xA3", Binary::Video),
        (b"\x00asm", Binary::Other),
        (b"\x7FELF", Binary::Other),
    ];
    TABLE
        .iter()
        .find(|(m, _)| bytes.starts_with(m))
        .map(|(_, b)| *b)
}

fn has(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|w| w == needle)
}

/// 说是二进制的 Content-Type。
fn binary_type(ct: &str) -> Option<Binary> {
    Some(match ct {
        "application/pdf" => Binary::Pdf,
        "application/zip"
        | "application/gzip"
        | "application/x-gzip"
        | "application/x-7z-compressed"
        | "application/vnd.rar" => Binary::Archive,
        "application/epub+zip" => Binary::Epub,
        "application/msword"
        | "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Binary::Word,
        "application/vnd.ms-excel"
        | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Binary::Excel,
        "application/vnd.ms-powerpoint"
        | "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            Binary::PowerPoint
        }
        _ if ct.starts_with("image/") => Binary::Image,
        _ if ct.starts_with("audio/") => Binary::Audio,
        _ if ct.starts_with("video/") => Binary::Video,
        _ => return None,
    })
}

/// 看不出来的二进制：参考 Content-Type 和扩展名。
fn by_type(ct: &str, ext: &str) -> Option<Binary> {
    binary_type(ct).or(match ext {
        "pdf" => Some(Binary::Pdf),
        "doc" | "docx" => Some(Binary::Word),
        "xls" | "xlsx" => Some(Binary::Excel),
        "ppt" | "pptx" => Some(Binary::PowerPoint),
        "zip" | "gz" | "tgz" | "7z" | "rar" => Some(Binary::Archive),
        _ => None,
    })
}

/// 开头一段转成小写文字（UTF-16 的先解码）。
fn lower_text(head: &[u8], utf16: bool) -> String {
    if utf16 {
        let (enc, bom) = encoding_rs::Encoding::for_bom(head).unwrap_or((encoding_rs::UTF_16LE, 2));
        enc.decode_without_bom_handling(&head[bom..])
            .0
            .to_lowercase()
    } else {
        String::from_utf8_lossy(head).to_ascii_lowercase()
    }
}

/// 整份看起来像 JSON：首尾是成对的括号。
fn looks_json(bytes: &[u8]) -> bool {
    let trimmed = bytes.trim_ascii();
    matches!(
        (trimmed.first(), trimmed.last()),
        (Some(b'{'), Some(b'}')) | (Some(b'['), Some(b']'))
    )
}

fn ext_of(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_wins_over_headers() {
        assert_eq!(
            sniff(b"<!doctype html><p>x", Some("image/png"), "a.png"),
            Ok(Kind::Html)
        );
        assert_eq!(
            sniff(b"%PDF-1.4", Some("text/html"), "a.html"),
            Err(Binary::Pdf)
        );
        assert_eq!(
            sniff(b"\r\n\x00\x9f junk %PDF-1.7\n", None, ""),
            Err(Binary::Pdf),
            "前面是垃圾字节"
        );
        assert_eq!(
            sniff(
                b"<!doctype html><title>What does %PDF-1.7 mean?</title>",
                None,
                ""
            ),
            Ok(Kind::Html),
            "网页里写着 %PDF- 的不算"
        );
        assert_eq!(
            sniff("文件开头是 %PDF-1.7".as_bytes(), None, ""),
            Ok(Kind::Text)
        );
        assert_eq!(
            sniff(b"plain words", Some("image/png"), ""),
            Err(Binary::Image),
            "像文字、看不出来：参考 Content-Type"
        );
        assert_eq!(
            sniff(b"plain words", Some("application/octet-stream"), ""),
            Ok(Kind::Text)
        );
    }

    #[test]
    fn text_kinds_use_hints_only_when_content_is_ambiguous() {
        assert_eq!(
            sniff(b"# Title\n\ntext", Some("text/markdown"), ""),
            Ok(Kind::Markdown)
        );
        assert_eq!(
            sniff(b"# Title\n\ntext", None, "https://x.example/README.md"),
            Ok(Kind::Markdown)
        );
        assert_eq!(sniff(b"# Title\n\ntext", None, ""), Ok(Kind::Text));
        assert_eq!(
            sniff(
                b"<div align=\"center\"><img src=\"logo.svg\"></div>\n\n# Rust\n",
                Some("text/plain; charset=utf-8"),
                "https://raw.example.com/rust/README.md"
            ),
            Ok(Kind::Markdown),
            "夹着 HTML 的 README 还是 Markdown"
        );
        assert_eq!(
            sniff(b"<!DOCTYPE html><html>", None, "page.md"),
            Ok(Kind::Html),
            "整页 HTML 不看扩展名"
        );
        assert_eq!(
            sniff(br#"{"html": "<div>x</div>"}"#, None, ""),
            Ok(Kind::Json),
            "JSON 里的 <div> 不算网页"
        );
        assert_eq!(sniff(b"[1, 2]", Some("text/plain"), ""), Ok(Kind::Json));
        assert_eq!(sniff(b"[link](x) and more", None, ""), Ok(Kind::Text));
        assert_eq!(
            sniff(b"<?xml version=\"1.0\"?><rss/>", None, ""),
            Ok(Kind::Xml)
        );
        assert_eq!(
            sniff(b"<?xml version=\"1.0\"?>\n<!DOCTYPE html><html>", None, ""),
            Ok(Kind::Html)
        );
        assert_eq!(
            sniff(b"<note>x</note>", Some("application/xml"), ""),
            Ok(Kind::Xml)
        );
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("<html><p>你好".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(sniff(&utf16, None, ""), Ok(Kind::Html));
    }

    #[test]
    fn media_and_archives() {
        assert_eq!(sniff(b"GIF89a....", None, ""), Err(Binary::Image));
        assert_eq!(
            sniff(b"RIFF\x00\x00\x00\x00WEBPVP8 ", None, ""),
            Err(Binary::Image)
        );
        assert_eq!(
            sniff(b"RIFF\x00\x00\x00\x00WAVEfmt ", None, ""),
            Err(Binary::Audio)
        );
        assert_eq!(
            sniff(b"\x00\x00\x00\x18ftypM4A \x00\x00", None, ""),
            Err(Binary::Audio)
        );
        assert_eq!(sniff(b"\x1A\x45\xDF\xA3....", None, ""), Err(Binary::Video));
        assert_eq!(
            sniff(b"7z\xBC\xAF\x27\x1C\x00\x04", None, ""),
            Err(Binary::Archive)
        );
        assert_eq!(
            sniff(&[0u8; 64], Some("application/pdf"), ""),
            Err(Binary::Pdf),
            "认不出来的二进制参考 Content-Type"
        );
        assert_eq!(
            sniff(&[0u8; 64], None, "https://x.example/a.docx?dl=1"),
            Err(Binary::Word),
            "再参考扩展名"
        );
    }
}
