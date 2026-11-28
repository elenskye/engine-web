//! arXiv 摘要页（`arxiv.org/abs/…`）：正文是摘要和一张「标签 | 值」表。去掉「Title:」「Abstract:」这些标签，
//! 表改成列表，全文的链接（PDF、HTML、TeX 源文件）留一行，提交记录一版一行。题名、作者、PDF 另在文献信息里。

use super::{Site, escape, label_table_to_list, text_of, unlink};
use dom_query::{NodeRef, Selection};
use regex::Regex;
use std::sync::OnceLock;

pub(super) const SITE: Site = Site {
    key: "arxiv",
    matches: |u| {
        matches!(
            u.host_str(),
            Some("arxiv.org" | "www.arxiv.org" | "export.arxiv.org")
        ) && u.path().starts_with("/abs/")
    },
    fix: None,
    root: &["#abs-outer", "#abs"],
    drop: &[
        ".subheader",
        ".header-breadcrumbs-mobile",
        "h1.title",
        // 摘要上面给手机看的「View PDF」按钮（下面「Full text」一行有）、arXivLabs 的工具页签、「谁是推荐人」
        ".mobile-submission-download",
        ".labstabs",
        ".endorsers",
        // DOI 旁边的「更多说明」小按钮和它的提示
        ".button-and-tooltip",
    ],
    shape: Some(shape),
};

fn shape(root: &NodeRef) {
    let all = Selection::from(*root);
    // 全文的链接先记下来，右栏（浏览、引用工具、书签）整个去掉
    let full_text: Vec<String> = all
        .select(".full-text a.abs-button")
        .nodes()
        .iter()
        .filter_map(|a| {
            let href = a.attr("href")?;
            Some(format!(
                r#"<a href="{}">{}</a>"#,
                escape(&href),
                escape(&text_of(a))
            ))
        })
        .collect();
    all.select(".extra-services").remove();
    for d in all.select(".dateline").nodes() {
        let text = text_of(d);
        let text = text.trim_start_matches('[').trim_end_matches(']').trim();
        d.replace_with_html(format!("<p>{}</p>", escape(text)));
    }
    for authors in all.select(".authors").nodes() {
        Selection::from(*authors).select(".descriptor").remove();
        unlink(authors);
        authors.replace_with_html(format!("<p>Authors: {}</p>", escape(&text_of(authors))));
    }
    for abs in all.select("blockquote.abstract").nodes() {
        Selection::from(*abs).select(".descriptor").remove();
        let inner = abs.inner_html();
        abs.replace_with_html(format!("<h2>Abstract</h2><p>{}</p>", inner.trim()));
    }
    for table in all.select(".metatable table").nodes() {
        label_table_to_list(table);
    }
    if !full_text.is_empty() {
        if let Some(meta) = all.select(".metatable").nodes().first() {
            meta.after_html(format!("<p>Full text: {}</p>", full_text.join(" · ")));
        }
    }
    for history in all.select(".submission-history").nodes() {
        submission_history(history);
    }
}

/// 提交记录：`<br>` 隔开的几行改成列表，去掉「[view email]」。
fn submission_history(node: &NodeRef) {
    static EMAIL: OnceLock<Regex> = OnceLock::new();
    let email = EMAIL.get_or_init(|| Regex::new(r"\s*\[\s*view email\s*\]").unwrap());
    let mut lines = vec![String::new()];
    let mut heading = String::new();
    for child in node.children() {
        if child.is_element() {
            match super::name_of(&child).as_str() {
                "br" => lines.push(String::new()),
                "h2" => heading = text_of(&child),
                _ => lines.last_mut().unwrap().push_str(&child.text()),
            }
        } else if child.is_text() {
            lines.last_mut().unwrap().push_str(&child.text());
        }
    }
    let items: String = lines
        .iter()
        .map(|l| {
            email
                .replace_all(l, "")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|l| !l.is_empty())
        .map(|l| format!("<li>{}</li>", escape(&l)))
        .collect();
    let heading = if heading.is_empty() {
        "Submission history".to_string()
    } else {
        heading
    };
    node.replace_with_html(format!("<h2>{}</h2><ul>{items}</ul>", escape(&heading)));
}
