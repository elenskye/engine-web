//! 合成样本：`tests/fixtures/html/` 里每份样本配一份 `<名字>.expect.json`，两种模式各跑一遍、对期望。
//! 期望的写法见 `tests/fixtures/html/README.md`。

use engine_web::{Article, ExtractOptions, Mode, extract};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const DIR: &str = "tests/fixtures/html";

struct Sample {
    name: String,
    path: PathBuf,
    expect: Value,
}

fn samples() -> Vec<Sample> {
    let dir = Path::new(DIR);
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let Some(name) = file.strip_suffix(".expect.json") else {
            continue;
        };
        let expect: Value = serde_json::from_slice(&fs::read(&path).unwrap())
            .unwrap_or_else(|e| panic!("{file} 不是合法的 JSON：{e}"));
        let sample = match expect["sample"].as_str() {
            Some(s) => dir.join(s),
            None => ["html", "md", "json", "xml", "txt"]
                .iter()
                .map(|ext| dir.join(format!("{name}.{ext}")))
                .find(|p| p.exists())
                .unwrap_or_else(|| panic!("{file} 旁边没有样本")),
        };
        out.push(Sample {
            name: name.to_string(),
            path: sample,
            expect,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn strs(v: &Value) -> Vec<&str> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

#[test]
fn every_sample_has_expectations() {
    let samples = samples();
    assert!(samples.len() >= 30, "样本太少：{}", samples.len());
    let dir = Path::new(DIR);
    let used: Vec<PathBuf> = samples.iter().map(|s| s.path.clone()).collect();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        if file.ends_with(".expect.json") || file == "README.md" {
            continue;
        }
        assert!(used.contains(&path), "{file} 没有 .expect.json");
    }
    for s in &samples {
        assert!(
            s.expect["about"].as_str().is_some_and(|a| !a.is_empty()),
            "{} 缺 about",
            s.name
        );
        assert!(
            s.expect["not_web"].is_string() || s.expect["kind"].is_string(),
            "{} 缺 kind 或 not_web",
            s.name
        );
    }
}

#[test]
fn legacy_encoded_samples_are_not_utf8() {
    for name in [
        "gbk-undeclared",
        "gbk-meta",
        "gbk-header",
        "big5-meta",
        "big5-undeclared",
        "sjis-undeclared",
        "latin-undeclared",
    ] {
        let bytes = fs::read(Path::new(DIR).join(format!("{name}.html"))).unwrap();
        assert!(std::str::from_utf8(&bytes).is_err(), "{name}");
    }
}

#[test]
fn samples_meet_their_expectations() {
    let mut failures = Vec::new();
    for s in samples() {
        let bytes = fs::read(&s.path).unwrap();
        let ct = s.expect["content_type"].as_str();
        let url = s.expect["url"].as_str().unwrap_or("");
        for mode in [Mode::Article, Mode::Full] {
            let opts = ExtractOptions {
                mode,
                ..Default::default()
            };
            let tag = format!("{} [{}]", s.name, mode.key());
            let result = extract(&bytes, ct, url, &opts);
            if let Some(key) = s.expect["not_web"].as_str() {
                match result {
                    Err(engine_web::Error::NotWeb(b)) if b.key() == key => {}
                    other => failures.push(format!(
                        "{tag}：应当是 not_web({key})，实际 {:?}",
                        other.map(|a| a.kind)
                    )),
                }
                continue;
            }
            match result {
                Ok(article) => check(
                    &tag,
                    &article,
                    &s.expect,
                    &s.expect[mode.key()],
                    &mut failures,
                ),
                Err(e) => failures.push(format!("{tag}：出错 {e}")),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} 处不符：\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn check(tag: &str, a: &Article, expect: &Value, mode: &Value, failures: &mut Vec<String>) {
    let mut fail = |m: String| failures.push(format!("{tag}：{m}"));
    if let Some(kind) = expect["kind"].as_str() {
        if a.kind.key() != kind {
            fail(format!("kind 应为 {kind}，实际 {}", a.kind.key()));
        }
    }
    if let Some(enc) = expect["encoding"].as_str() {
        if a.encoding != enc {
            fail(format!("encoding 应为 {enc}，实际 {}", a.encoding));
        }
    }
    for (field, got) in [
        ("title", &a.title),
        ("byline", &a.byline),
        ("published", &a.published),
        ("site", &a.site),
        ("lang", &a.lang),
    ] {
        if let Some(want) = expect[field].as_str() {
            if got.as_deref() != Some(want) {
                fail(format!("{field} 应为 {want:?}，实际 {got:?}"));
            }
        }
    }
    if let Some(js) = expect["needs_js"].as_bool() {
        if a.needs_js != js {
            fail(format!("needs_js 应为 {js}"));
        }
    }
    if let Some(rule) = expect.get("rule") {
        if a.rule != rule.as_str() {
            fail(format!("rule 应为 {rule}，实际 {:?}", a.rule));
        }
    }
    if let Some(next) = expect.get("next_page") {
        if a.next_page.as_deref() != next.as_str() {
            fail(format!("next_page 应为 {next}，实际 {:?}", a.next_page));
        }
    }
    if let Some(want) = expect.get("paper") {
        check_paper(a.paper.as_ref(), want, &mut fail);
    }
    if let Some(want) = expect.get("images") {
        check_images(&a.images, want, &mut fail);
    }
    for want in strs(&mode["contains"]) {
        if !a.markdown.contains(want) {
            fail(format!("正文里没有 {want:?}"));
        }
    }
    for bad in strs(&mode["excludes"]) {
        if a.markdown.contains(bad) || a.blocks.iter().any(|b| b.text.contains(bad)) {
            fail(format!("正文里不该有 {bad:?}"));
        }
    }
    if let Some(exact) = mode["exact"].as_str() {
        if a.markdown != exact {
            fail(format!("正文应当逐字是 {exact:?}\n实际 {:?}", a.markdown));
        }
    }
    if let Some(max) = mode["max_chars"].as_u64() {
        let n = a.markdown.trim().chars().count() as u64;
        if n > max {
            fail(format!(
                "正文应当最多 {max} 字，实际 {n} 字：{:?}",
                a.markdown
            ));
        }
    }
    for url in strs(&mode["links"]) {
        if !a.links.iter().any(|l| l.url == url) {
            fail(format!("links 里没有 {url}"));
        }
    }
    for want in mode["blocks"].as_array().into_iter().flatten() {
        let text = want["contains"].as_str().unwrap();
        let Some(block) = a.blocks.iter().find(|b| b.text.contains(text)) else {
            fail(format!("没有一段包含 {text:?}"));
            continue;
        };
        if let Some(kind) = want["kind"].as_str() {
            if block.kind.key() != kind {
                fail(format!(
                    "{text:?} 那段应为 {kind}，实际 {}",
                    block.kind.key()
                ));
            }
        }
        if let Some(path) = want["path"].as_array() {
            let path: Vec<&str> = path.iter().filter_map(Value::as_str).collect();
            if block.heading_path != path {
                fail(format!(
                    "{text:?} 那段的标题路径应为 {path:?}，实际 {:?}",
                    block.heading_path
                ));
            }
        }
    }
    for (i, b) in a.blocks.iter().enumerate() {
        if b.paragraph as usize != i + 1 {
            fail(format!("段号不连续：第 {} 个是 {}", i + 1, b.paragraph));
            break;
        }
        if b.text.trim().is_empty() {
            fail(format!("第 {} 段是空的", b.paragraph));
        }
    }
    if a.markdown.trim().is_empty() != a.blocks.is_empty() {
        fail("正文和分段一个空一个不空".into());
    }
}

/// `paper`：`null` 就要没有；对象里写了哪项查哪项（`authors`、`keywords` 逐个相等，`affiliations` 要出现在某位作者的机构里，
/// `abstract` 是摘要里要有的一句，`ids` 是 `{编号种类: 值}`）。
fn check_paper(got: Option<&engine_web::Paper>, want: &Value, fail: &mut impl FnMut(String)) {
    let Some(want) = want.as_object() else {
        if got.is_some() {
            fail(format!("不该有 paper，实际 {got:?}"));
        }
        return;
    };
    let Some(p) = got else {
        fail("应当有 paper".into());
        return;
    };
    for (field, value) in [
        ("title", &p.title),
        ("journal", &p.journal),
        ("publisher", &p.publisher),
        ("volume", &p.volume),
        ("issue", &p.issue),
        ("pages", &p.pages),
        ("number", &p.number),
        ("published", &p.published),
        ("doi", &p.doi),
        ("pdf_url", &p.pdf_url),
    ] {
        if let Some(w) = want.get(field) {
            if value.as_deref() != w.as_str() {
                fail(format!("paper.{field} 应为 {w}，实际 {value:?}"));
            }
        }
    }
    if let Some(w) = want.get("authors") {
        let names: Vec<&str> = p.authors.iter().map(|a| a.name.as_str()).collect();
        if names != strs(w) {
            fail(format!("paper.authors 应为 {:?}，实际 {names:?}", strs(w)));
        }
    }
    for aff in strs(&want.get("affiliations").cloned().unwrap_or_default()) {
        if !p
            .authors
            .iter()
            .any(|a| a.affiliations.iter().any(|x| x == aff))
        {
            fail(format!("没有哪位作者的机构是 {aff:?}"));
        }
    }
    if let Some(w) = want.get("keywords") {
        if p.keywords.iter().map(String::as_str).collect::<Vec<_>>() != strs(w) {
            fail(format!(
                "paper.keywords 应为 {:?}，实际 {:?}",
                strs(w),
                p.keywords
            ));
        }
    }
    if let Some(w) = want.get("abstract").and_then(Value::as_str) {
        if !p.abstract_text.as_deref().is_some_and(|a| a.contains(w)) {
            fail(format!("摘要里没有 {w:?}，实际 {:?}", p.abstract_text));
        }
    }
    if let Some(ids) = want.get("ids").and_then(Value::as_object) {
        for (scheme, value) in ids {
            if !p
                .ids
                .iter()
                .any(|i| i.scheme == scheme && Some(i.value.as_str()) == value.as_str())
            {
                fail(format!(
                    "paper.ids 里没有 {scheme}={value}，实际 {:?}",
                    p.ids
                ));
            }
        }
    }
}

/// `images`：数组里每一项都要在 `images` 里找得到（写了哪项比哪项）；`{"count": n}` 只比张数。
fn check_images(got: &[engine_web::Image], want: &Value, fail: &mut impl FnMut(String)) {
    if let Some(n) = want.get("count").and_then(Value::as_u64) {
        if got.len() as u64 != n {
            fail(format!("应当有 {n} 张图，实际 {}", got.len()));
        }
    }
    for w in want
        .get("list")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let ok = got.iter().any(|i| {
            w.get("url")
                .is_none_or(|u| u.as_str() == Some(i.url.as_str()))
                && w.get("alt").is_none_or(|v| v.as_str() == i.alt.as_deref())
                && w.get("caption")
                    .is_none_or(|v| v.as_str() == i.caption.as_deref())
                && w.get("paragraph")
                    .is_none_or(|v| v.as_u64() == i.paragraph.map(u64::from))
        });
        if !ok {
            fail(format!("images 里没有 {w}，实际 {got:?}"));
        }
    }
}
