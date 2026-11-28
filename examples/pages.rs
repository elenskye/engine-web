//! 真实网页的批量核对（v0.2 Phase 3，只在本机）：对 `local/pages/` 里的每份快照跑两种模式，打印、写进 `local/out/`。
//!
//! ```text
//! cargo run --example pages                       # 读 local/pages/，写 local/out/
//! cargo run --example pages -- tests/fixtures/html  # 换个目录（比如合成样本）
//! cargo run --example pages -- <目录> --out <目录>
//! ```
//!
//! 快照：`<名字>.<扩展名>` 是原件的字节；旁边可以放 `<名字>.<扩展名>.meta.json`：
//! `{"url": "https://…", "content_type": "text/html; charset=gbk", "fetched": "2026-11-27"}`（都可省）。
//! `urls.txt`、`*.meta.json`、`*.expect.json`、`README.md` 和点开头的文件不当快照。
//! 人看的是：有没有把导航当正文、有没有丢表格和链接、有没有乱码。

use engine_web::{Article, ExtractOptions, Mode, extract};
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let mut args = std::env::args().skip(1);
    let mut input = PathBuf::from("local/pages");
    let mut out = PathBuf::from("local/out");
    while let Some(arg) = args.next() {
        if arg == "--out" {
            out = PathBuf::from(args.next().expect("--out 后面要跟目录"));
        } else {
            input = PathBuf::from(arg);
        }
    }
    let pages = snapshots(&input);
    if pages.is_empty() {
        println!(
            "没有真实网页：{} 不存在或是空的。把快照放进去再跑（做法见 HANDOFF.md「真实网页」）。",
            input.display()
        );
        return;
    }
    fs::create_dir_all(&out).expect("建不了输出目录");
    let mut summary = String::from(
        "| 文件 | 类型 | 编码 | 标题 | article 字数 | full 字数 | 段数 | needs_js | 开头 |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for page in &pages {
        let bytes = fs::read(page).expect("读不了快照");
        let (url, content_type) = meta(page);
        let name = page.file_name().unwrap().to_string_lossy().to_string();
        let run = |mode| {
            extract(
                &bytes,
                content_type.as_deref(),
                &url,
                &ExtractOptions {
                    mode,
                    ..Default::default()
                },
            )
        };
        println!("== {name}  {url}");
        match (run(Mode::Article), run(Mode::Full)) {
            (Ok(a), Ok(f)) => {
                let head = first_lines(&a.markdown, 3);
                println!(
                    "   标题：{}\n   类型 {} · 编码 {} · article {} 字 · full {} 字 · {} 段 · needs_js {}\n   作者 {} · 日期 {} · 站名 {} · 语言 {} · 链接 {} 个",
                    a.title.as_deref().unwrap_or("（无）"),
                    a.kind.key(),
                    a.encoding,
                    chars(&a),
                    chars(&f),
                    a.blocks.len(),
                    a.needs_js,
                    a.byline.as_deref().unwrap_or("—"),
                    a.published.as_deref().unwrap_or("—"),
                    a.site.as_deref().unwrap_or("—"),
                    a.lang.as_deref().unwrap_or("—"),
                    a.links.len(),
                );
                for line in head.lines() {
                    println!("   │ {line}");
                }
                write(&out, &name, "article.md", &a, &url);
                write(&out, &name, "full.txt", &f, &url);
                summary.push_str(&format!(
                    "| {name} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                    a.kind.key(),
                    a.encoding,
                    cell(a.title.as_deref().unwrap_or("")),
                    chars(&a),
                    chars(&f),
                    a.blocks.len(),
                    a.needs_js,
                    cell(&head.replace('\n', " ⏎ ")),
                ));
            }
            (Err(e), _) | (_, Err(e)) => {
                println!("   {e}（{}）", e.kind());
                summary.push_str(&format!(
                    "| {name} | {} | | {} | | | | | |\n",
                    e.kind(),
                    cell(&e.to_string())
                ));
            }
        }
    }
    fs::write(out.join("summary.md"), summary).expect("写不了 summary.md");
    println!(
        "\n{} 页，结果在 {}/（每页的 *.article.md、*.full.txt，总表 summary.md）",
        pages.len(),
        out.display()
    );
}

fn snapshots(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            !(name.starts_with('.')
                || name == "urls.txt"
                || name == "README.md"
                || name.ends_with(".meta.json")
                || name.ends_with(".expect.json"))
        })
        .collect();
    out.sort();
    out
}

/// `<快照>.meta.json` 里的网址和 Content-Type；合成样本旁边的 `<名字>.expect.json` 也认。
fn meta(page: &Path) -> (String, Option<String>) {
    let name = page.file_name().unwrap().to_string_lossy().to_string();
    let stem = name
        .rsplit_once('.')
        .map(|(s, _)| s.to_string())
        .unwrap_or(name.clone());
    let candidates = [
        page.with_file_name(format!("{name}.meta.json")),
        page.with_file_name(format!("{stem}.expect.json")),
    ];
    for path in candidates {
        if let Ok(text) = fs::read_to_string(path) {
            return (
                field(&text, "url").unwrap_or_default(),
                field(&text, "content_type"),
            );
        }
    }
    (String::new(), None)
}

/// 从一行行的 JSON 里取一个字符串字段（不为一个例子引 JSON 库）。
fn field(json: &str, key: &str) -> Option<String> {
    let at = json.find(&format!("\"{key}\""))?;
    let rest = json[at + key.len() + 2..]
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn write(out: &Path, name: &str, suffix: &str, a: &Article, url: &str) {
    let mut text = format!(
        "<!-- {url} · {} · {} · {} 段 · needs_js {} -->\n",
        a.kind.key(),
        a.encoding,
        a.blocks.len(),
        a.needs_js
    );
    if let Some(t) = &a.title {
        text.push_str(&format!("<!-- 标题：{t} -->\n"));
    }
    if let Some(rule) = a.rule {
        text.push_str(&format!("<!-- 规则：{rule} -->\n"));
    }
    if let Some(p) = &a.paper {
        let authors: Vec<&str> = p.authors.iter().map(|a| a.name.as_str()).collect();
        text.push_str(&format!(
            "<!-- 文献：{} · 作者 {} 位（{}） · {} · DOI {} · 日期 {} · PDF {} · 关键词 {} · 摘要 {} 字 -->\n",
            p.title.as_deref().unwrap_or("（无标题）"),
            authors.len(),
            clip(&authors.join("; "), 80),
            [&p.journal, &p.volume, &p.issue, &p.pages, &p.number]
                .iter()
                .filter_map(|v| v.as_deref())
                .collect::<Vec<_>>()
                .join(" / "),
            p.doi.as_deref().unwrap_or("—"),
            p.published.as_deref().unwrap_or("—"),
            p.pdf_url.as_deref().unwrap_or("—"),
            p.keywords.join("; "),
            p.abstract_text.as_deref().map_or(0, |t| t.chars().count()),
        ));
    }
    text.push('\n');
    text.push_str(&a.markdown);
    text.push_str("\n\n<!-- 分段 -->\n");
    for b in &a.blocks {
        let first = b.text.lines().next().unwrap_or("");
        text.push_str(&format!(
            "{} [{}] {}\n",
            b.label(),
            b.kind.key(),
            clip(first, 60)
        ));
    }
    fs::write(out.join(format!("{name}.{suffix}")), text).expect("写不了结果");
}

fn chars(a: &Article) -> usize {
    a.markdown.chars().filter(|c| !c.is_whitespace()).count()
}

fn first_lines(text: &str, n: usize) -> String {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .take(n)
        .map(|l| clip(l, 80))
        .collect::<Vec<_>>()
        .join("\n")
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
}

fn cell(s: &str) -> String {
    clip(&s.replace('|', "\\|"), 60)
}
