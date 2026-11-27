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
    for name in ["gbk-undeclared", "gbk-meta", "gbk-header", "big5-meta"] {
        let bytes = fs::read(Path::new(DIR).join(format!("{name}.html"))).unwrap();
        assert!(std::str::from_utf8(&bytes).is_err(), "{name}");
    }
}

#[test]
#[ignore = "v0.2 Phase 2 实现取正文后打开"]
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
