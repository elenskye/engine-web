//! 整理 `article` 模式转出来的 Markdown（Peck 的 `tidy_markdown` 加 Phase 3 的规则）。

use regex::Regex;
use std::sync::OnceLock;
use url::Url;

/// 链接目标：不含空白和括号的字、转义的字、或一对括号（`Foo_(bar)`）。
const TARGET: &str = r"(?:[^()\s\\]|\\.|\([^()\s]*\))+";
/// 链接标题：`"…"`，里面可以有转义的引号。
const TITLE: &str = r#"(?:\s+"(?:[^"\\]|\\.)*")?"#;

/// - 图片只留说明（没说明的、`data:` 的整个去掉）；
/// - `javascript:` 链接只留文字；
/// - 链接改成绝对网址，去掉链接标题（维基的标题和链接文字一样，Nature 把整条参考文献放在里面，都是重复）；
/// - 不需要的转义还原；小标题的锚点、`¶` 永久链接、只剩记号的空列表项去掉；连着的空行收成一个。
pub(super) fn tidy(md: &str, base: Option<&Url>) -> String {
    static IMAGE: OnceLock<Regex> = OnceLock::new();
    static SCRIPT_LINK: OnceLock<Regex> = OnceLock::new();
    static LINK: OnceLock<Regex> = OnceLock::new();
    static PERMALINK: OnceLock<Regex> = OnceLock::new();
    static SELF_LINK: OnceLock<Regex> = OnceLock::new();
    static EMPTY_ITEM: OnceLock<Regex> = OnceLock::new();
    static BLANK: OnceLock<Regex> = OnceLock::new();
    let image =
        IMAGE.get_or_init(|| Regex::new(&format!(r"!\[([^\]]*)\]\(({TARGET})?{TITLE}\)")).unwrap());
    let script_link = SCRIPT_LINK.get_or_init(|| {
        Regex::new(r"(?i)\[([^\]]*)\]\(javascript:(?:[^()]|\([^()]*\))*\)").unwrap()
    });
    let link = LINK.get_or_init(|| Regex::new(&format!(r"\]\(({TARGET}){TITLE}\)")).unwrap());
    // 小标题的锚点：`## [标题](#标题)` 只留文字，`[¶](#…)`、`[#](#…)` 这类永久链接去掉（MDN、mdBook、Sphinx）
    let permalink =
        PERMALINK.get_or_init(|| Regex::new(r"\[(?:¶|\\?#|§|🔗)\]\(#[^)\s]*\)").unwrap());
    let self_link = SELF_LINK
        .get_or_init(|| Regex::new(r"(?m)^(#{1,6}[ \t]+)\[([^\]]+)\]\(#[^)\s]*\)[ \t]*$").unwrap());
    // 内容去掉以后只剩记号的空列表项、空小标题
    let empty_item = EMPTY_ITEM
        .get_or_init(|| Regex::new(r"(?m)^[ \t]*(?:[-*+]|\d{1,9}[.)]|#{1,6})[ \t]*$").unwrap());
    let blank = BLANK.get_or_init(|| Regex::new(r"\n{3,}").unwrap());

    let text = image.replace_all(md, |c: &regex::Captures| {
        let alt = c[1].trim();
        let data = c.get(2).is_some_and(|t| t.as_str().starts_with("data:"));
        if alt.is_empty() || data {
            String::new()
        } else {
            format!("（图：{alt}）")
        }
    });
    let text = script_link.replace_all(&text, "$1");
    let text = link.replace_all(&text, |c: &regex::Captures| {
        format!("]({})", absolute(&c[1], base))
    });
    let text = unescape(&text);
    let text = permalink.replace_all(&text, "");
    let text = self_link.replace_all(&text, "$1$2");
    let text = empty_item.replace_all(&text, "");
    blank.replace_all(text.trim(), "\n\n").to_string()
}

/// 相对网址补成绝对的；`#…` 和带协议的（`https:`、`mailto:`，照网址的规矩看开头）不动。
pub(super) fn absolute(target: &str, base: Option<&Url>) -> String {
    let has_scheme = target.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    if target.starts_with('#') || has_scheme {
        return target.to_string();
    }
    base.and_then(|b| b.join(target).ok())
        .map(|u| u.to_string())
        .unwrap_or_else(|| target.to_string())
}

/// dom_query 把 `.`、`(`、`#` 这些都转义了；在那个位置不需要转义的还原，读着顺、和原文对得上。
/// 留着的：会改变 Markdown 含义的（`\*`、`\_`、`` \` ``、`\[`、`\]`、`\<`、`\\`、行首的 `1\.`、`\#`、`\+`、`\>`、表格里的 `\|`、`\![`），
/// 和链接网址里的 `\(`、`\)`、`\\`。
fn unescape(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut in_fence = false;
    for (i, line) in md.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
        }
        if in_fence || !line.contains('\\') {
            out.push_str(line);
            continue;
        }
        let table = t.starts_with('|');
        let chars: Vec<char> = line.chars().collect();
        // 链接的网址 `](网址)` 里：`\"`、`\(`、`\)`、`\\` 要留着，别的照常还原
        let mut in_link = false;
        let mut j = 0;
        while j < chars.len() {
            let c = chars[j];
            let escape = c == '\\'
                && chars
                    .get(j + 1)
                    .is_some_and(|n| !in_link || !matches!(n, '"' | '(' | ')' | '\\'));
            let Some(&next) = chars.get(j + 1).filter(|_| escape) else {
                if c == ']' && chars.get(j + 1) == Some(&'(') {
                    in_link = true;
                } else if c == ')' && in_link && chars.get(j.wrapping_sub(1)) != Some(&'\\') {
                    in_link = false;
                }
                out.push(c);
                j += 1;
                continue;
            };
            // 前面只有空白和引用记号：算行首
            let at_start = chars[..j].iter().all(|c| c.is_whitespace() || *c == '>');
            let digits_before = chars[..j]
                .iter()
                .rev()
                .take_while(|c| c.is_ascii_digit())
                .count();
            let list_number = digits_before > 0
                && chars[..j - digits_before]
                    .iter()
                    .all(|c| c.is_whitespace() || *c == '>')
                && chars.get(j + 2).is_none_or(|c| c.is_whitespace());
            let keep = match next {
                '.' => list_number,
                '!' => chars.get(j + 2) == Some(&'['),
                '(' | ')' | '{' | '}' | '"' => false,
                '#' | '+' | '>' => at_start,
                '|' => table,
                _ => true,
            };
            if keep {
                out.push(c);
            }
            out.push(next);
            j += 2;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://w.example.com/a/b.html").unwrap()
    }

    #[test]
    fn links_become_absolute_without_titles_and_images_keep_only_captions() {
        let md = "看[这里](../c/d.html \"标题\")和[锚点](#x)、[外站](https://o.example/p)、[维基](/wiki/Help:Contents \"Help:Contents\")。\n\n\n\n![](a.png)![图注](b.png \"图\")![x](data:image/png;base64,AA)";
        assert_eq!(
            tidy(md, Some(&base())),
            "看[这里](https://w.example.com/c/d.html)和[锚点](#x)、[外站](https://o.example/p)、[维基](https://w.example.com/wiki/Help:Contents)。\n\n（图：图注）"
        );
        assert_eq!(
            tidy("[相对](c.html)", None),
            "[相对](c.html)",
            "没有网址就不补"
        );
        // 网址里成对的括号、标题里转义的引号（Nature 的参考文献）
        assert_eq!(
            tidy(
                r#"[ISBN](https://en.example.org/wiki/ISBN_(identifier) "ISBN (identifier)") 和 [1](#ref-CR1 "Thompson, M. C. \"Advances\" \(2020\).")"#,
                Some(&base())
            ),
            "[ISBN](https://en.example.org/wiki/ISBN_(identifier)) 和 [1](#ref-CR1)"
        );
        // 带协议的不补（空的 mailto: 也是）
        assert_eq!(
            tidy("[信](mailto:) [电话](tel:+100)", Some(&base())),
            "[信](mailto:) [电话](tel:+100)"
        );
        assert_eq!(
            tidy("-\n\n    -\n- 有字\n1.\n2. 也有\n\n##\n\n## 标题", None),
            "- 有字\n\n2. 也有\n\n## 标题"
        );
        assert_eq!(
            tidy("## [选项](#options)\n\n文字[¶](#p \"Permalink\")", None),
            "## 选项\n\n文字"
        );
    }

    #[test]
    fn needless_escapes_are_undone() {
        assert_eq!(
            unescape(r#"他说 \"好\"，见[链接](https://x.example/a) 和 \"引号\""#),
            r#"他说 "好"，见[链接](https://x.example/a) 和 "引号""#
        );
        assert_eq!(
            unescape(r"[a](https://x.example/b\)c) 之后 \(括号\)"),
            r"[a](https://x.example/b\)c) 之后 (括号)"
        );
        assert_eq!(
            unescape(r"全长 12\.6 公里\(约\)，见 \{x\}\! 和 C\# a\>b"),
            "全长 12.6 公里(约)，见 {x}! 和 C# a>b"
        );
        assert_eq!(unescape(r"3\.14 是数"), "3.14 是数");
        assert_eq!(
            unescape(r"1\. 不是列表 \# \+ \* \_ \` \[ \] \< \\"),
            r"1\. 不是列表 # + \* \_ \` \[ \] \< \\"
        );
        assert_eq!(
            unescape("\\# 不是标题\n> \\+ 不是列表\n\\> 不是引用"),
            "\\# 不是标题\n> \\+ 不是列表\n\\> 不是引用"
        );
        assert_eq!(unescape(r"\![不是图](x) | a\|b"), r"\![不是图](x) | a|b");
        assert_eq!(unescape(r"| a\|b | 1\.5 |"), r"| a\|b | 1.5 |");
        assert_eq!(
            unescape("```\n保留 \\. 原样\n```\n12\\.5"),
            "```\n保留 \\. 原样\n```\n12.5"
        );
    }
}
