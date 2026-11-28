//! 分段：一段、表格一行、列表一项、一个代码块、一个小标题各算一段，全文连续编号；标题路径不含文章标题。
//!
//! Markdown（`article` 模式的结果、Markdown 原文）按行切；`full` 模式的段由 `html` 模块走树切；纯文本按空行切。
//! 段的文字去掉 Markdown 记号，find 比的是它。

use crate::{Block, BlockKind};
use regex::Regex;
use std::sync::OnceLock;

/// 还没编号的一段。`level` 只对小标题有意义（1–6）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Raw {
    pub kind: BlockKind,
    pub level: u8,
    pub text: String,
}

impl Raw {
    pub fn new(kind: BlockKind, text: String) -> Self {
        Self {
            kind,
            level: 0,
            text,
        }
    }

    pub fn heading(level: u8, text: String) -> Self {
        Self {
            kind: BlockKind::Heading,
            level,
            text,
        }
    }
}

/// 编号、算标题路径。开头的一级标题是文章标题（和 `title` 一样，或全文只有它一个一级标题），不进路径。
pub(crate) fn number(raw: Vec<Raw>, title: Option<&str>) -> Vec<Block> {
    let raw: Vec<Raw> = raw
        .into_iter()
        .filter(|r| !r.text.trim().is_empty())
        .collect();
    let h1s = raw
        .iter()
        .filter(|r| r.kind == BlockKind::Heading && r.level == 1)
        .count();
    let title_at = raw
        .iter()
        .position(|r| r.kind == BlockKind::Heading)
        .filter(|&i| {
            let r = &raw[i];
            r.level == 1 && (h1s == 1 || title.is_some_and(|t| same_words(t, &r.text)))
        });
    let mut stack: Vec<(u8, String)> = Vec::new();
    let mut out = Vec::with_capacity(raw.len());
    for (i, r) in raw.into_iter().enumerate() {
        let heading_path: Vec<String> = stack.iter().map(|(_, t)| t.clone()).collect();
        let (heading_path, push) = if r.kind != BlockKind::Heading || Some(i) == title_at {
            (heading_path, false)
        } else {
            while stack.last().is_some_and(|(l, _)| *l >= r.level) {
                stack.pop();
            }
            (stack.iter().map(|(_, t)| t.clone()).collect(), true)
        };
        if push {
            stack.push((r.level, r.text.clone()));
        }
        out.push(Block {
            heading_path,
            paragraph: out.len() as u32 + 1,
            kind: r.kind,
            text: r.text,
            page: None,
        });
    }
    out
}

fn same_words(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.split_whitespace().collect::<String>().to_lowercase();
    norm(a) == norm(b)
}

/// Markdown → 段。
pub(crate) fn from_markdown(md: &str) -> Vec<Raw> {
    let mut out = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut para_kind = BlockKind::Paragraph;
    let flush = |para: &mut Vec<String>, kind: BlockKind, out: &mut Vec<Raw>| {
        if !para.is_empty() {
            let text = para
                .iter()
                .map(|l| strip_inline(l))
                .collect::<Vec<_>>()
                .join("\n");
            out.push(Raw::new(kind, text.trim().to_string()));
            para.clear();
        }
    };
    let mut lines = md.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        // 代码块
        if let Some(fence) = fence_of(line) {
            flush(&mut para, para_kind, &mut out);
            let mut code = Vec::new();
            for next in lines.by_ref() {
                if next.trim_start().starts_with(fence)
                    && next
                        .trim()
                        .trim_start_matches(fence.chars().next().unwrap())
                        .is_empty()
                {
                    break;
                }
                code.push(next);
            }
            out.push(Raw::new(
                BlockKind::Code,
                code.join("\n").trim_end().to_string(),
            ));
            continue;
        }
        if trimmed.is_empty() {
            flush(&mut para, para_kind, &mut out);
            continue;
        }
        if let Some((level, text)) = atx_heading(line) {
            flush(&mut para, para_kind, &mut out);
            out.push(Raw::heading(level, strip_inline(text)));
            continue;
        }
        // Setext 标题：一行字下面一行 === 或 ---
        if para.len() == 1
            && para_kind == BlockKind::Paragraph
            && trimmed.len() >= 2
            && (trimmed.chars().all(|c| c == '=') || trimmed.chars().all(|c| c == '-'))
        {
            let text = strip_inline(&para.pop().unwrap());
            out.push(Raw::heading(
                if trimmed.starts_with('=') { 1 } else { 2 },
                text,
            ));
            continue;
        }
        if is_rule(trimmed) {
            flush(&mut para, para_kind, &mut out);
            continue;
        }
        if trimmed.starts_with('|') {
            flush(&mut para, para_kind, &mut out);
            if !is_table_rule(trimmed) {
                let cells: Vec<String> = table_cells(trimmed)
                    .iter()
                    .map(|c| strip_inline(c))
                    .collect();
                out.push(Raw::new(
                    BlockKind::TableRow,
                    cells.join("\t").trim_end_matches('\t').to_string(),
                ));
            }
            continue;
        }
        if let Some(item) = list_item(line) {
            flush(&mut para, para_kind, &mut out);
            para_kind = BlockKind::ListItem;
            para.push(item.to_string());
            continue;
        }
        let quoted = quote_body(line);
        if let Some(body) = quoted {
            if para_kind != BlockKind::Paragraph {
                flush(&mut para, para_kind, &mut out);
            }
            if body.trim().is_empty() {
                flush(&mut para, BlockKind::Paragraph, &mut out);
            } else {
                para_kind = BlockKind::Paragraph;
                para.push(body.to_string());
            }
            continue;
        }
        // 不是新块的一行接在上一段（或上一项）后面，缩不缩进都算（CommonMark 的「懒续行」：项里用 <br> 断行的文字）
        if para.is_empty() {
            para_kind = BlockKind::Paragraph;
        }
        para.push(line.to_string());
    }
    flush(&mut para, para_kind, &mut out);
    out
}

/// 开头的 YAML 头（`---` … `---` 或 `...`）：交回里面的 `title` 和头后面的正文。没有头就原样交回。
pub(crate) fn front_matter(md: &str) -> (Option<String>, &str) {
    let first = md.lines().next().unwrap_or("");
    if first.trim_end() != "---" {
        return (None, md);
    }
    let mut at = first.len() + 1;
    let mut title = None;
    for line in md.get(at..).unwrap_or("").split_inclusive('\n') {
        at += line.len();
        let t = line.trim_end();
        if t == "---" || t == "..." {
            return (title, md.get(at..).unwrap_or(""));
        }
        if let Some(v) = t.strip_prefix("title:") {
            let v = v.trim().trim_matches(['"', '\'']).trim();
            if !v.is_empty() {
                title = Some(v.to_string());
            }
        }
    }
    (None, md)
}

/// 纯文本 → 段（按空行）。
pub(crate) fn from_text(text: &str) -> Vec<Raw> {
    let mut out = Vec::new();
    let mut para: Vec<&str> = Vec::new();
    for line in text.lines().chain(std::iter::once("")) {
        let line = line.trim();
        if line.is_empty() {
            if !para.is_empty() {
                out.push(Raw::new(BlockKind::Paragraph, para.join("\n")));
                para.clear();
            }
        } else {
            para.push(line);
        }
    }
    out
}

/// Markdown 原文里的 http(s) 链接（不含图片），相对的按 `base` 补全，同一网址只留第一次。
pub(crate) fn links_in_markdown(md: &str, base: Option<&url::Url>) -> Vec<crate::Link> {
    static LINK: OnceLock<Regex> = OnceLock::new();
    let re = LINK.get_or_init(|| Regex::new(r"(!?)\[([^\]]*)\]\(([^)\s]+)").unwrap());
    let mut out: Vec<crate::Link> = Vec::new();
    for c in re.captures_iter(md) {
        if &c[1] == "!" {
            continue;
        }
        let target = &c[3];
        let url = match url::Url::parse(target) {
            Ok(u) => u,
            Err(_) => match base.and_then(|b| b.join(target).ok()) {
                Some(u) => u,
                None => continue,
            },
        };
        let url = url.to_string();
        if (url.starts_with("http://") || url.starts_with("https://"))
            && !out.iter().any(|l| l.url == url)
        {
            out.push(crate::Link {
                text: strip_inline(&c[2]),
                url,
            });
        }
    }
    out
}

fn fence_of(line: &str) -> Option<&'static str> {
    let t = line.trim_start();
    if line.len() - t.len() > 3 {
        return None;
    }
    if t.starts_with("```") {
        Some("```")
    } else if t.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

fn atx_heading(line: &str) -> Option<(u8, &str)> {
    let t = line.trim_start();
    if line.len() - t.len() > 3 {
        return None;
    }
    let level = t.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &t[level..];
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    let text = rest.trim().trim_end_matches('#').trim_end();
    Some((level as u8, text))
}

fn is_rule(t: &str) -> bool {
    let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && ["-", "*", "_"]
            .iter()
            .any(|c| compact.chars().all(|x| x.to_string() == *c))
}

fn is_table_rule(t: &str) -> bool {
    t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
}

/// `| a | b \| c |` → `["a", "b | c"]`（去掉首尾的竖线，转义的竖线留在格子里）。
fn table_cells(row: &str) -> Vec<String> {
    let inner = row.strip_prefix('|').unwrap_or(row);
    let inner = if inner.ends_with('|') && !inner.ends_with("\\|") {
        &inner[..inner.len() - 1]
    } else {
        inner
    };
    let mut cells = vec![String::new()];
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cells.last_mut().unwrap().push('|');
                chars.next();
            }
            '|' => cells.push(String::new()),
            _ => cells.last_mut().unwrap().push(c),
        }
    }
    cells.into_iter().map(|c| c.trim().to_string()).collect()
}

fn list_item(line: &str) -> Option<&str> {
    static ITEM: OnceLock<Regex> = OnceLock::new();
    let re = ITEM.get_or_init(|| Regex::new(r"^\s*(?:[-*+]|\d{1,9}[.)])\s+(.*)$").unwrap());
    re.captures(line).map(|c| c.get(1).unwrap().as_str())
}

fn quote_body(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let mut rest = t.strip_prefix('>')?;
    // 嵌套的引用：> > 文字
    loop {
        let r = rest.strip_prefix(' ').unwrap_or(rest);
        match r.strip_prefix('>') {
            Some(more) => rest = more,
            None => return Some(r),
        }
    }
}

/// 去掉行内的 Markdown 记号：链接只留文字，图片只留说明，强调、行内代码、删除线去掉记号，反斜杠转义还原。
pub(crate) fn strip_inline(s: &str) -> String {
    static ESCAPE: OnceLock<Regex> = OnceLock::new();
    static RULES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let escape = ESCAPE.get_or_init(|| Regex::new(r"\\([!-/:-@\[-`{-~])").unwrap());
    let rules = RULES.get_or_init(|| {
        [
            (r"!\[([^\]]*)\]\([^)]*\)", "$1"),
            (
                r"\[([^\]]*)\]\((?:[^()\s]|\([^()\s]*\))*(?:\s+\x22[^\x22]*\x22)?\)",
                "$1",
            ),
            (r"\[([^\]]+)\]\[[^\]]*\]", "$1"),
            (r"<((?:https?|mailto):[^>\s]+)>", "$1"),
            (r"\*\*\*([^*]+)\*\*\*", "$1"),
            (r"\*\*([^*]+)\*\*", "$1"),
            (r"__([^_]+)__", "$1"),
            (r"\*([^*\s][^*]*)\*", "$1"),
            (r"(^|[^\w])_([^_\s][^_]*)_($|\W)", "$1$2$3"),
            (r"~~([^~]+)~~", "$1"),
        ]
        .into_iter()
        .map(|(re, to)| (Regex::new(re).unwrap(), to))
        .collect()
    });
    // 转义的字符和行内代码里的字先换成占位（私用区），记号去完再还原
    let hide = |t: &str| {
        t.chars()
            .map(|c| {
                if c.is_ascii_punctuation() {
                    char::from_u32(0xF0000 + c as u32).unwrap()
                } else {
                    c
                }
            })
            .collect::<String>()
    };
    let out = escape.replace_all(s.trim_end(), |c: &regex::Captures| hide(&c[1]));
    let mut out = code_spans(&out, hide);
    for (re, to) in rules {
        out = re.replace_all(&out, *to).into_owned();
    }
    let out: String = out
        .chars()
        .map(|c| {
            if ('\u{F0021}'..='\u{F007E}').contains(&c) {
                char::from_u32(c as u32 - 0xF0000).unwrap()
            } else {
                c
            }
        })
        .collect();
    out.trim().to_string()
}

/// 行内代码：一串反引号到下一串一样长的反引号；里面的字交给 `inner`，反引号去掉。
fn code_spans(s: &str, inner: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('`') {
        let run = rest[start..].chars().take_while(|c| *c == '`').count();
        let after = &rest[start + run..];
        let ticks = "`".repeat(run);
        let close = after
            .match_indices(ticks.as_str())
            .map(|(i, _)| i)
            .find(|&i| {
                !after[i + run..].starts_with('`') && (i == 0 || !after[..i].ends_with('`'))
            });
        match close {
            Some(end) => {
                out.push_str(&rest[..start]);
                let body = &after[..end];
                let body = if body.starts_with(' ') && body.ends_with(' ') && body.trim() != "" {
                    &body[1..body.len() - 1]
                } else {
                    body
                };
                out.push_str(&inner(body));
                rest = &after[end + run..];
            }
            None => {
                out.push_str(&rest[..start + run]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks(md: &str, title: Option<&str>) -> Vec<(String, String, &'static str)> {
        number(from_markdown(md), title)
            .into_iter()
            .map(|b| (b.label(), b.text, b.kind.key()))
            .collect()
    }

    #[test]
    fn markdown_blocks_have_paths_and_continuous_numbers() {
        let md = "# 文章标题\n\n开头一段，**加粗**和[链接](https://x.example/a)。\n\n## 引言\n\n第一段\n第一段的第二行\n\n### 方法\n\n- 一项\n- 二项\n  续行\n\n| 列 | 值 |\n| --- | --- |\n| `a` | 1 \\| 2 |\n\n```rust\nfn main() {}\n\n// 空行也在代码里\n```\n\n## 结论\n\n> 引用的话\n\n收尾。";
        assert_eq!(
            blocks(md, Some("文章标题")),
            vec![
                ("第 1 段".into(), "文章标题".into(), "heading"),
                (
                    "第 2 段".into(),
                    "开头一段，加粗和链接。".into(),
                    "paragraph"
                ),
                ("第 3 段".into(), "引言".into(), "heading"),
                (
                    "〈引言〉第 4 段".into(),
                    "第一段\n第一段的第二行".into(),
                    "paragraph"
                ),
                ("〈引言〉第 5 段".into(), "方法".into(), "heading"),
                ("〈引言 › 方法〉第 6 段".into(), "一项".into(), "list_item"),
                (
                    "〈引言 › 方法〉第 7 段".into(),
                    "二项\n续行".into(),
                    "list_item"
                ),
                (
                    "〈引言 › 方法〉第 8 段".into(),
                    "列\t值".into(),
                    "table_row"
                ),
                (
                    "〈引言 › 方法〉第 9 段".into(),
                    "a\t1 | 2".into(),
                    "table_row"
                ),
                (
                    "〈引言 › 方法〉第 10 段".into(),
                    "fn main() {}\n\n// 空行也在代码里".into(),
                    "code"
                ),
                ("第 11 段".into(), "结论".into(), "heading"),
                ("〈结论〉第 12 段".into(), "引用的话".into(), "paragraph"),
                ("〈结论〉第 13 段".into(), "收尾。".into(), "paragraph"),
            ]
        );
    }

    #[test]
    fn title_heading_only_when_it_is_the_title() {
        // 两个一级标题、都不是文章标题：都进路径
        let b = blocks("# 一\n\n甲\n\n# 二\n\n乙", Some("别的"));
        assert_eq!(b[1].0, "〈一〉第 2 段");
        assert_eq!(b[3].0, "〈二〉第 4 段");
        // 只有一个一级标题：就是文章标题
        let b = blocks("# 标题\n\n## 小节\n\n正文", None);
        assert_eq!(b[2].0, "〈小节〉第 3 段");
        // 没有一级标题：二级标题照常进路径
        let b = blocks("## 小节\n\n正文", Some("小节"));
        assert_eq!(b[1].0, "〈小节〉第 2 段");
    }

    #[test]
    fn list_items_keep_their_broken_lines() {
        // 列表项里用 <br> 断行，转出来下一行不缩进：还是这一项
        let b = blocks(
            "- 国家自然科学  \n基金委员会官方\n- 科金谭  \n哔哩哔哩\n\n下一段",
            None,
        );
        assert_eq!(
            b.iter().map(|b| (b.1.as_str(), b.2)).collect::<Vec<_>>(),
            [
                ("国家自然科学\n基金委员会官方", "list_item"),
                ("科金谭\n哔哩哔哩", "list_item"),
                ("下一段", "paragraph")
            ]
        );
    }

    #[test]
    fn inline_marks_are_stripped() {
        assert_eq!(
            strip_inline(r"My favorite is [Duck Duck Go](https://duckduckgo.com)\."),
            "My favorite is Duck Duck Go."
        );
        assert_eq!(
            strip_inline(r#"[a](https://x.example "t") and ![图](p.png)"#),
            "a and 图"
        );
        assert_eq!(
            strip_inline("[Foo (bar)](https://w.example/wiki/Foo_(bar))"),
            "Foo (bar)"
        );
        assert_eq!(
            strip_inline("***both*** **b** *i* `code` ~~del~~ __u__"),
            "both b i code del u"
        );
        assert_eq!(
            strip_inline("snake_case_name and _em_"),
            "snake_case_name and em"
        );
        assert_eq!(
            strip_inline(r"It\`s 1\. \*not\* bold"),
            "It`s 1. *not* bold"
        );
        assert_eq!(strip_inline("``a `tick` b``"), "a `tick` b");
    }

    #[test]
    fn front_matter_gives_the_title_and_is_not_a_block() {
        let md = "---\ntitle: 'Quick Start'\nsidebar: learn\n---\n\n<Intro>\n\nWelcome.\n";
        let (title, body) = front_matter(md);
        assert_eq!(title.as_deref(), Some("Quick Start"));
        assert_eq!(body, "\n<Intro>\n\nWelcome.\n");
        assert_eq!(front_matter("# 没有头\n"), (None, "# 没有头\n"));
        assert_eq!(front_matter("---\n没有收尾"), (None, "---\n没有收尾"));
    }

    #[test]
    fn text_splits_on_blank_lines() {
        let b = number(from_text("一\n二\n\n\n三  \n"), None);
        assert_eq!(
            b.iter().map(|b| b.text.as_str()).collect::<Vec<_>>(),
            ["一\n二", "三"]
        );
    }
}
