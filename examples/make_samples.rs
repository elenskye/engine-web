//! 生成 `tests/fixtures/html/` 里不是 UTF-8 的网页和「不是网页」的样本（PDF、图片、Office、压缩包、音视频）。
//!
//! `cargo run --example make_samples`：重写这些文件和它们的 `.expect.json`。UTF-8 的样本是手写的，不经这里。

use std::fs;
use std::path::Path;

const DIR: &str = "tests/fixtures/html";

/// 没声明编码的 GBK 中文页（Peck、Pomelo 现在会解成乱码）。
const GBK_UNDECLARED: &str = r#"<html>
<head><title>示例县图书馆冬季开放时间调整</title></head>
<body>
<div class="nav"><a href="/">首页</a> <a href="/news/">馆内动态</a> <a href="/guide/">读者指南</a></div>
<div class="content">
<h1>示例县图书馆冬季开放时间调整</h1>
<p>自 2026 年 12 月 1 日起，县图书馆实行冬季开放时间：周二至周日 9:00—17:30，周一闭馆。少儿阅览室在寒假期间每天延长开放一小时，到 18:30 闭馆。</p>
<p>自助借还机和还书箱全天开放。读者可以通过微信公众号续借图书，每本书可续借一次，续借期为三十天。逾期未还的图书每天收取滞纳金一角。</p>
<p>寒假期间，二楼自习室每天提供三百个座位，读者凭读者证在入口处的预约机上选座，每次最多预约四小时，离开超过半小时座位自动释放。三楼的地方文献室照常开放，查阅古籍需提前一天预约。</p>
<p>如有疑问，请拨打服务台电话咨询，或到一楼总服务台当面询问。</p>
</div>
<div class="foot">示例县图书馆 版权所有</div>
</body>
</html>
"#;

/// meta 声明 gb2312、字节是 GBK（超出 GB2312 的字也要对：「镕」「堃」）。
const GBK_META: &str = r#"<html>
<head>
<meta http-equiv="Content-Type" content="text/html; charset=gb2312">
<title>示例大学 2026 年优秀毕业论文名单公示</title>
</head>
<body>
<div id="article">
<h1>示例大学 2026 年优秀毕业论文名单公示</h1>
<p>经学院推荐、学校评审，现将 2026 年本科优秀毕业论文名单公示如下。公示期为五个工作日，期间如有异议，请以书面形式向教务处反映，并写明真实姓名和联系方式。</p>
<table>
<tr><th>学院</th><th>作者</th><th>论文题目</th></tr>
<tr><td>材料学院</td><td>王镕</td><td>高熵合金的低温韧性研究</td></tr>
<tr><td>文学院</td><td>李堃</td><td>宋代笔记中的市井饮食</td></tr>
</table>
<p>教务处 2026 年 6 月 10 日</p>
</div>
</body>
</html>
"#;

/// 编码只在 HTTP 头里写（Content-Type: text/html; charset=GBK），网页里没写。
const GBK_HEADER: &str = r#"<html>
<head><title>停水通知</title></head>
<body>
<h1>停水通知</h1>
<p>因主干管网维修，11 月 30 日 8:00 至 20:00，东城区和平路以北的居民小区将暂停供水。请各位居民提前储水，关好家中水龙头，以免恢复供水时跑水。</p>
<p>恢复供水初期水质可能略有浑浊，放水几分钟后即可正常使用。给大家带来的不便，敬请谅解。</p>
<p>示例市自来水公司</p>
</body>
</html>
"#;

/// 声明了 Big5 的繁体中文页。
const BIG5_META: &str = r#"<html>
<head>
<meta charset="big5">
<title>範例市立美術館秋季特展</title>
</head>
<body>
<div class="main">
<h1>範例市立美術館秋季特展</h1>
<p>本館秋季特展「山與海之間」即日起至明年一月底展出，集結四十位藝術家的水墨、油畫與攝影作品，呈現臺灣東部的地景變化。展覽期間每週六下午有導覽，免費參加，名額有限，請提早報名。</p>
<p>開放時間為週二至週日上午九點至下午五點，週一休館。學生憑證件享半價優惠，六十五歲以上長者免費入場。</p>
</div>
</body>
</html>
"#;

/// 没声明编码的 Big5 页（网址是 .com，没有地区提示）。
const BIG5_UNDECLARED: &str = r#"<html>
<head><title>社區大學春季課程開始報名</title></head>
<body>
<h1>社區大學春季課程開始報名</h1>
<p>本學期共開設八十門課程，內容涵蓋語言、電腦、園藝、烹飪與攝影。課程每週上課一次，每次兩小時，學費依課程時數計算，六十五歲以上長者與身心障礙者可享學費減半優惠。</p>
<p>報名方式分為網路報名與現場報名兩種。網路報名請至本校網站填寫表單，現場報名請攜帶身分證件至一樓服務台辦理。名額有限，額滿為止。</p>
</body>
</html>
"#;

/// 没声明编码的 Shift_JIS 页（.jp）。
const SJIS_UNDECLARED: &str = r#"<html>
<head><title>市立図書館の冬季開館時間について</title></head>
<body>
<h1>市立図書館の冬季開館時間について</h1>
<p>十二月一日から三月末まで、市立図書館の開館時間は午前九時から午後五時半までとなります。毎週月曜日と年末年始は休館いたします。</p>
<p>返却ポストは休館日も利用できます。予約した資料は、連絡を受けてから一週間以内に受け取ってください。ご理解とご協力をお願いいたします。</p>
</body>
</html>
"#;

/// 没声明编码的 windows-1252 法语页（.fr）。
const LATIN_UNDECLARED: &str = r#"<html>
<head><title>Horaires d'hiver de la médiathèque</title></head>
<body>
<h1>Horaires d'hiver de la médiathèque</h1>
<p>À partir du 1er décembre, la médiathèque sera ouverte du mardi au dimanche de 9 h à 17 h 30. Le prêt reste gratuit pour tous les habitants de la commune, et les pénalités de retard sont supprimées pendant l'hiver.</p>
<p>Les élèves peuvent réserver une salle de travail à l'accueil. Un café littéraire aura lieu chaque premier samedi du mois, à partir de 15 h, dans la salle polyvalente.</p>
</body>
</html>
"#;

fn main() {
    let dir = Path::new(DIR);
    encoded(
        dir,
        "gbk-undeclared",
        GBK_UNDECLARED,
        encoding_rs::GBK,
        r#"
  "about": "没声明编码的 GBK 中文页：不是合法的 UTF-8，chardetng 探测",
  "url": "http://lib.example.cn/news/2026/winter.html",
  "kind": "html",
  "encoding": "GBK",
  "title": "示例县图书馆冬季开放时间调整",
  "needs_js": false,
  "article": {
    "contains": ["周二至周日 9:00—17:30，周一闭馆", "逾期未还的图书每天收取滞纳金一角"],
    "excludes": ["馆内动态", "版权所有"]
  },
  "full": {"contains": ["首页 馆内动态 读者指南", "续借期为三十天"], "excludes": []}"#,
    );
    encoded(
        dir,
        "gbk-meta",
        GBK_META,
        encoding_rs::GBK,
        r#"
  "about": "meta 声明 gb2312、字节是 GBK（GB2312 以外的字「镕」「堃」也要对）；表格",
  "url": "https://jwc.example.edu.cn/info/2026/0610.htm",
  "kind": "html",
  "encoding": "GBK",
  "title": "示例大学 2026 年优秀毕业论文名单公示",
  "needs_js": false,
  "article": {
    "contains": ["公示期为五个工作日", "| 材料学院 | 王镕 | 高熵合金的低温韧性研究 |", "| 文学院 | 李堃 | 宋代笔记中的市井饮食 |"],
    "excludes": []
  },
  "full": {"contains": ["材料学院\t王镕\t高熵合金的低温韧性研究"], "excludes": []}"#,
    );
    encoded(
        dir,
        "gbk-header",
        GBK_HEADER,
        encoding_rs::GBK,
        r#"
  "about": "编码只在 HTTP 头里写（charset=GBK）",
  "url": "http://water.example.cn/notice/1130.html",
  "content_type": "text/html; charset=GBK",
  "kind": "html",
  "encoding": "GBK",
  "title": "停水通知",
  "needs_js": false,
  "article": {"contains": ["东城区和平路以北的居民小区将暂停供水"], "excludes": []},
  "full": {"contains": ["放水几分钟后即可正常使用"], "excludes": []}"#,
    );
    encoded(
        dir,
        "big5-meta",
        BIG5_META,
        encoding_rs::BIG5,
        r#"
  "about": "meta 声明 Big5 的繁体中文页",
  "url": "https://museum.example.tw/exhibitions/autumn.html",
  "kind": "html",
  "encoding": "Big5",
  "title": "範例市立美術館秋季特展",
  "needs_js": false,
  "article": {"contains": ["本館秋季特展「山與海之間」即日起至明年一月底展出", "六十五歲以上長者免費入場"], "excludes": []},
  "full": {"contains": ["呈現臺灣東部的地景變化"], "excludes": []}"#,
    );

    encoded(
        dir,
        "big5-undeclared",
        BIG5_UNDECLARED,
        encoding_rs::BIG5,
        r#"
  "about": "没声明编码的 Big5 繁体页，网址是 .com（没有地区提示）：chardetng 探测",
  "url": "https://cu.example.com/news/spring.html",
  "kind": "html",
  "encoding": "Big5",
  "title": "社區大學春季課程開始報名",
  "needs_js": false,
  "article": {"contains": ["六十五歲以上長者與身心障礙者可享學費減半優惠"], "excludes": []},
  "full": {"contains": ["名額有限，額滿為止。"], "excludes": []}"#,
    );
    encoded(
        dir,
        "sjis-undeclared",
        SJIS_UNDECLARED,
        encoding_rs::SHIFT_JIS,
        r#"
  "about": "没声明编码的 Shift_JIS 日文页：chardetng 探测",
  "url": "https://library.example.jp/news/winter.html",
  "kind": "html",
  "encoding": "Shift_JIS",
  "title": "市立図書館の冬季開館時間について",
  "needs_js": false,
  "article": {"contains": ["市立図書館の開館時間は午前九時から午後五時半までとなります"], "excludes": []},
  "full": {"contains": ["返却ポストは休館日も利用できます。"], "excludes": []}"#,
    );
    encoded(
        dir,
        "latin-undeclared",
        LATIN_UNDECLARED,
        encoding_rs::WINDOWS_1252,
        r#"
  "about": "没声明编码的 windows-1252 法语页：chardetng 探测（退 GB18030 会把重音字母解成汉字）",
  "url": "https://mediatheque.example.fr/horaires.html",
  "kind": "html",
  "encoding": "windows-1252",
  "title": "Horaires d'hiver de la médiathèque",
  "needs_js": false,
  "article": {"contains": ["À partir du 1er décembre, la médiathèque sera ouverte"], "excludes": []},
  "full": {"contains": ["Un café littéraire aura lieu chaque premier samedi du mois"], "excludes": []}"#,
    );
    binary(
        dir,
        "paper.pdf",
        &pdf(),
        "pdf",
        "PDF（Content-Type 说是 text/html 也不信）",
        Some("text/html"),
    );
    binary(dir, "pixel.png", &png(), "image", "PNG 图片", None);
    binary(
        dir,
        "photo.jpg",
        &jpeg(),
        "image",
        "JPEG 图片",
        Some("application/octet-stream"),
    );
    binary(dir, "logo.svg", br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>"#, "image", "SVG 图片", None);
    binary(
        dir,
        "bundle.zip",
        &zip(&[("readme.txt", b"hello")]),
        "archive",
        "ZIP 压缩包",
        None,
    );
    binary(
        dir,
        "report.docx",
        &zip(&[
            ("[Content_Types].xml", b"<Types/>"),
            ("word/document.xml", b"<w:document/>"),
        ]),
        "word",
        "Word 文档（docx）",
        None,
    );
    binary(
        dir,
        "sheet.xlsx",
        &zip(&[
            ("[Content_Types].xml", b"<Types/>"),
            ("xl/workbook.xml", b"<workbook/>"),
        ]),
        "excel",
        "Excel 表格（xlsx）",
        None,
    );
    binary(
        dir,
        "slides.pptx",
        &zip(&[
            ("[Content_Types].xml", b"<Types/>"),
            ("ppt/presentation.xml", b"<p:presentation/>"),
        ]),
        "powerpoint",
        "PowerPoint（pptx）",
        None,
    );
    binary(
        dir,
        "book.epub",
        &zip(&[
            ("mimetype", b"application/epub+zip"),
            ("META-INF/container.xml", b"<container/>"),
        ]),
        "epub",
        "EPUB 电子书",
        None,
    );
    binary(
        dir,
        "legacy.doc",
        &ole("WordDocument"),
        "word",
        "旧版 Word（.doc，OLE 复合文档）",
        None,
    );
    binary(
        dir,
        "legacy.xls",
        &ole("Workbook"),
        "excel",
        "旧版 Excel（.xls，OLE 复合文档）",
        None,
    );
    binary(dir, "clip.mp4", &mp4(b"isom"), "video", "MP4 视频", None);
    binary(
        dir,
        "song.mp3",
        b"ID3\x04\x00\x00\x00\x00\x00\x00\xff\xfb\x90\x00",
        "audio",
        "MP3 音频（ID3 头）",
        None,
    );
    binary(
        dir,
        "data.gz",
        b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x00\x03\x4b\x4c\x4a\x06\x00",
        "archive",
        "gzip 压缩文件",
        None,
    );
    binary(
        dir,
        "blob.bin",
        &(0u8..=255).cycle().take(1024).collect::<Vec<_>>(),
        "other",
        "认不出来的二进制",
        None,
    );
    println!("已写入 {DIR}");
}

fn encoded(dir: &Path, name: &str, html: &str, enc: &'static encoding_rs::Encoding, expect: &str) {
    let (bytes, _, unmappable) = enc.encode(html);
    assert!(!unmappable, "{name} 里有 {} 编不了的字", enc.name());
    assert!(
        std::str::from_utf8(&bytes).is_err(),
        "{name} 应当不是合法的 UTF-8"
    );
    fs::write(dir.join(format!("{name}.html")), &bytes).unwrap();
    fs::write(
        dir.join(format!("{name}.expect.json")),
        format!("{{{expect}\n}}\n"),
    )
    .unwrap();
}

fn binary(
    dir: &Path,
    file: &str,
    bytes: &[u8],
    key: &str,
    about: &str,
    content_type: Option<&str>,
) {
    fs::write(dir.join(file), bytes).unwrap();
    let stem = file
        .rsplit_once('.')
        .map(|(s, e)| format!("{s}-{e}"))
        .unwrap();
    let ct = content_type
        .map(|c| format!("\n  \"content_type\": \"{c}\","))
        .unwrap_or_default();
    let expect = format!(
        "{{\n  \"about\": \"不是网页：{about}\",\n  \"sample\": \"{file}\",\n  \"url\": \"https://files.example.com/{file}\",{ct}\n  \"not_web\": \"{key}\"\n}}\n"
    );
    fs::write(dir.join(format!("{stem}.expect.json")), expect).unwrap();
}

/// 一份最小的、结构完整的 PDF（一页，一行字）。
fn pdf() -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
        "<< /Length 44 >>\nstream\nBT /F1 12 Tf 20 50 Td (Hello PDF) Tj ET\nendstream",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// 1×1 的透明 PNG。
fn png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]
}

/// JPEG 的开头（SOI + JFIF APP0），后面补些字节。
fn jpeg() -> Vec<u8> {
    let mut out = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x00,
    ];
    out.extend((0u8..64).map(|i| i.wrapping_mul(37)));
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

/// MP4 的 ftyp 盒子。
fn mp4(brand: &[u8; 4]) -> Vec<u8> {
    let mut out = vec![0x00, 0x00, 0x00, 0x18, b'f', b't', b'y', b'p'];
    out.extend_from_slice(brand);
    out.extend_from_slice(&[0x00, 0x00, 0x02, 0x00]);
    out.extend_from_slice(b"isomiso2");
    out.extend_from_slice(&[0x00, 0x00, 0x00, 0x08, b'f', b'r', b'e', b'e']);
    out
}

/// OLE 复合文档：文件头 + 一个目录扇区，里面有一个 UTF-16LE 的流名。
fn ole(stream: &str) -> Vec<u8> {
    let mut out = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    out.resize(512, 0);
    let mut entry: Vec<u8> = "Root Entry"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    entry.resize(128, 0);
    out.extend_from_slice(&entry);
    let mut entry: Vec<u8> = stream.encode_utf16().flat_map(u16::to_le_bytes).collect();
    entry.resize(128, 0);
    out.extend_from_slice(&entry);
    out.resize(1536, 0);
    out
}

/// 只存不压的 ZIP（够让类型判断认出里面的文件名）。
fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let header = |sig: u32, central: bool| {
            let mut h = sig.to_le_bytes().to_vec();
            if central {
                h.extend_from_slice(&20u16.to_le_bytes());
            }
            h.extend_from_slice(&20u16.to_le_bytes()); // 需要的版本
            h.extend_from_slice(&0u16.to_le_bytes()); // 标志
            h.extend_from_slice(&0u16.to_le_bytes()); // 不压缩
            h.extend_from_slice(&0u16.to_le_bytes()); // 时间
            h.extend_from_slice(&0x5B21u16.to_le_bytes()); // 日期 2025-09-01
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes()); // 扩展字段
            if central {
                h.extend_from_slice(&[0; 8]); // 注释长、盘号、内部属性
                h.extend_from_slice(&0u32.to_le_bytes()); // 外部属性
                h.extend_from_slice(&offset.to_le_bytes());
            }
            h.extend_from_slice(name.as_bytes());
            h
        };
        out.extend_from_slice(&header(0x0403_4b50, false));
        out.extend_from_slice(data);
        central.extend_from_slice(&header(0x0201_4b50, true));
    }
    let start = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
