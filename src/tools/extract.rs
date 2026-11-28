//! `web_extract`（本机文件 → 正文）、`web_find`（在网页里找字），和 `web_fetch` 共用的「正文分段交回」。

use super::{DATA_NOTE, Web, tool_error, wrap};
use crate::{Article, ExtractOptions, Mode, extract, fetch, find};
use engine_core::call::{Ctx, Output, Quote, ToolError};
use engine_core::find::{Found, Match, Query};
use serde_json::{Value, json};

/// 参数里的 `mode`。
pub(super) fn mode_of(args: &Value) -> Mode {
    if args["mode"] == "full" {
        Mode::Full
    } else {
        Mode::Article
    }
}

/// 字节 → 正文；正文为空交 `no_article`。
pub(super) fn article_of(
    bytes: &[u8],
    content_type: Option<&str>,
    url: &str,
    mode: Mode,
) -> Result<Article, ToolError> {
    let opts = ExtractOptions {
        mode,
        ..Default::default()
    };
    let article = extract(bytes, content_type, url, &opts).map_err(tool_error)?;
    if article.markdown.trim().is_empty() {
        return Err(ToolError::of("no_article"));
    }
    Ok(article)
}

/// `url` 参数：给了就要是完整的 http(s) 网址。
fn url_arg(args: &Value) -> Result<Option<String>, ToolError> {
    let url = args["url"]
        .as_str()
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_string);
    if let Some(u) = &url {
        if !url::Url::parse(u).is_ok_and(|u| matches!(u.scheme(), "http" | "https")) {
            return Err(ToolError::new(
                "bad_args",
                "url 要是完整的 http(s) 网址。",
                "给网页原来的网址（用来补全相对链接），不知道就不传。",
                false,
            ));
        }
    }
    Ok(url)
}

/// 读 App 交的文件、取正文。
fn load_file(args: &Value, ctx: &Ctx<'_>) -> Result<(Article, Option<String>, Mode), ToolError> {
    let url = url_arg(args)?;
    let mode = mode_of(args);
    let bytes = ctx.files.read(args["file"].as_str().unwrap_or_default())?;
    ctx.cancel.check()?;
    let article = article_of(
        &bytes,
        args["content_type"].as_str(),
        url.as_deref().unwrap_or(""),
        mode,
    )?;
    ctx.cancel.check()?;
    Ok((article, url, mode))
}

pub(super) fn web_extract(
    _: &Web,
    args: &Value,
    ctx: &Ctx<'_>,
    _: Option<&Quote>,
) -> Result<Output, ToolError> {
    let (article, url, mode) = load_file(args, ctx)?;
    let head = url.iter().map(|u| format!("网址：{u}")).collect();
    render(&article, url.as_deref(), head, mode, args, json!({}))
}

/// 正文分段交回（`web_extract`、`web_fetch` 共用）。`head` 是标题下面另加的几行，`more` 是 `data` 里另加的字段。
pub(super) fn render(
    article: &Article,
    url: Option<&str>,
    head: Vec<String>,
    mode: Mode,
    args: &Value,
    more: Value,
) -> Result<Output, ToolError> {
    let total = article.markdown.chars().count();
    let offset = args["offset"].as_u64().unwrap_or(0) as usize;
    let max_chars = args["max_chars"].as_u64().unwrap_or(20_000) as usize;
    if offset > 0 && offset >= total {
        return Err(ToolError::new(
            "bad_args",
            "offset 超过了正文的长度。",
            format!("正文一共 {total} 字，offset 要小于它。"),
            false,
        ));
    }
    let (segment, end) = segment(&article.markdown, offset, max_chars);
    let truncated = end < total;

    let mut text = String::new();
    if let Some(t) = &article.title {
        text += &format!("标题：{t}\n");
    }
    for line in head {
        text += &line;
        text.push('\n');
    }
    let paper_lines = article
        .paper
        .as_ref()
        .map(|p| paper_lines(p, article.published.as_deref()))
        .unwrap_or_default();
    // 有文献信息时作者在那几行里列，这一行不重复
    let byline = if article
        .paper
        .as_ref()
        .is_some_and(|p| !p.authors.is_empty())
    {
        &None
    } else {
        &article.byline
    };
    let meta: Vec<String> = [
        ("作者", byline),
        ("日期", &article.published),
        ("站名", &article.site),
        ("语言", &article.lang),
    ]
    .iter()
    .filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}：{v}")))
    .collect();
    if !meta.is_empty() {
        text += &format!("{}\n", meta.join(" · "));
    }
    for line in &paper_lines {
        text += line;
        text.push('\n');
    }
    let how = if mode == Mode::Full {
        "全部文字"
    } else {
        "正文"
    };
    text += &format!(
        "类型：{} · 编码：{} · {how}一共 {total} 字、{} 段\n",
        article.kind.key(),
        article.encoding,
        article.blocks.len()
    );
    if article.needs_js {
        text += "正文很短而有脚本：内容可能要开 JavaScript 才出来，下面的不一定全。\n";
    }
    text += DATA_NOTE;
    text.push('\n');
    text += &wrap(url, segment);
    if truncated {
        text += &format!(
            "\n（这是第 {}–{end} 字，一共 {total} 字；接着取：offset {end}。）",
            offset + 1
        );
    }

    let mut data = json!({
        "kind": article.kind.key(),
        "encoding": article.encoding,
        "mode": mode.key(),
        "needs_js": article.needs_js,
        "chars": total,
        "offset": offset,
        "end": end,
        "truncated": truncated,
        "blocks": article.blocks.len(),
        "markdown": segment,
        "links": article.links.iter().map(|l| json!({"text": l.text, "url": l.url})).collect::<Vec<_>>(),
    });
    for (key, value) in [
        ("title", &article.title),
        ("byline", &article.byline),
        ("published", &article.published),
        ("site", &article.site),
        ("lang", &article.lang),
    ] {
        if let Some(v) = value {
            data[key] = json!(v);
        }
    }
    if let Some(u) = url {
        data["url"] = json!(u);
    }
    if let Some(rule) = article.rule {
        data["rule"] = json!(rule);
    }
    if let Some(paper) = &article.paper {
        data["paper"] = json!(paper);
    }
    if !article.images.is_empty() {
        data["images"] = json!(article.images);
    }
    if let Some(next) = &article.next_page {
        data["next_page"] = json!(next);
    }
    if !article.pages.is_empty() {
        data["pages"] = json!(article.pages);
    }
    if let Value::Object(more) = more {
        for (k, v) in more {
            data[k] = v;
        }
    }
    let next = truncated.then(|| {
        let mut next = args.clone();
        next["offset"] = json!(end);
        next
    });
    Ok(Output {
        text,
        data,
        next,
        ..Output::default()
    })
}

/// 列几位作者（多的写「等 N 位」，全部在 `data.paper.authors`）。
const SHOW_AUTHORS: usize = 10;
/// 列几个机构。
const SHOW_AFFILIATIONS: usize = 5;

/// 文献信息给模型的几行（`decisions/0008`）：作者和机构、刊物、编号、PDF、关键词。摘要不重复（正文里有），
/// 发表日期和上面「日期」一样时也不重复（`shown_date`）。
fn paper_lines(p: &crate::Paper, shown_date: Option<&str>) -> Vec<String> {
    let mut lines = vec!["文献：".to_string()];
    if !p.authors.is_empty() {
        let names: Vec<&str> = p
            .authors
            .iter()
            .take(SHOW_AUTHORS)
            .map(|a| a.name.as_str())
            .collect();
        let more = p.authors.len().saturating_sub(SHOW_AUTHORS);
        let tail = if more > 0 {
            format!(" 等 {} 位", p.authors.len())
        } else {
            String::new()
        };
        lines.push(format!("- 作者：{}{tail}", names.join("; ")));
        let mut affiliations: Vec<&str> = Vec::new();
        for a in p.authors.iter().flat_map(|a| &a.affiliations) {
            if !affiliations.contains(&a.as_str()) {
                affiliations.push(a);
            }
        }
        if !affiliations.is_empty() {
            let more = affiliations.len().saturating_sub(SHOW_AFFILIATIONS);
            let tail = if more > 0 {
                format!(" 等 {} 个", affiliations.len())
            } else {
                String::new()
            };
            let shown: Vec<&str> = affiliations.into_iter().take(SHOW_AFFILIATIONS).collect();
            lines.push(format!("- 机构：{}{tail}", shown.join("; ")));
        }
    }
    let mut source: Vec<String> = Vec::new();
    if let Some(j) = &p.journal {
        source.push(j.clone());
    }
    for (label, value) in [
        ("卷", &p.volume),
        ("期", &p.issue),
        ("页", &p.pages),
        ("编号", &p.number),
    ] {
        if let Some(v) = value {
            source.push(format!("{label} {v}"));
        }
    }
    if let Some(publisher) = p
        .publisher
        .as_ref()
        .filter(|x| Some(*x) != p.journal.as_ref())
    {
        source.push(format!("出版 {publisher}"));
    }
    if let Some(d) = p.published.as_deref().filter(|d| Some(*d) != shown_date) {
        source.push(format!("发表 {d}"));
    }
    if !source.is_empty() {
        lines.push(format!("- 出处：{}", source.join("，")));
    }
    let mut ids: Vec<String> = p.doi.iter().map(|d| format!("DOI {d}")).collect();
    ids.extend(
        p.ids
            .iter()
            .map(|i| format!("{} {}", id_label(i.scheme), i.value)),
    );
    if !ids.is_empty() {
        lines.push(format!("- 编号：{}", ids.join(" · ")));
    }
    if let Some(pdf) = &p.pdf_url {
        lines.push(format!("- PDF：{pdf}"));
    }
    if !p.keywords.is_empty() {
        lines.push(format!("- 关键词：{}", p.keywords.join("; ")));
    }
    if lines.len() == 1 {
        return Vec::new();
    }
    lines
}

/// 编号种类给人看的写法。
fn id_label(scheme: &str) -> &str {
    match scheme {
        "arxiv" => "arXiv",
        "pmid" => "PMID",
        "pmcid" => "PMCID",
        "issn" => "ISSN",
        "isbn" => "ISBN",
        other => other,
    }
}

/// 从第 `offset` 个字起最多取 `max_chars` 个字；没到结尾时尽量断在段落（空行）或行尾。交回这一段和它的结尾（第几个字，不含）。
fn segment(md: &str, offset: usize, max_chars: usize) -> (&str, usize) {
    let byte_at = |chars: usize| md.char_indices().nth(chars).map_or(md.len(), |(i, _)| i);
    let start = byte_at(offset);
    let hard_end = byte_at(offset + max_chars);
    if hard_end >= md.len() {
        return (&md[start..], md.chars().count());
    }
    let window = &md[start..hard_end];
    let half = window.len() / 2;
    let cut = window
        .rfind("\n\n")
        .filter(|&i| i >= half)
        .map(|i| i + 2)
        .or_else(|| window.rfind('\n').filter(|&i| i >= half).map(|i| i + 1))
        .unwrap_or(window.len());
    let piece = &md[start..start + cut];
    (piece, offset + piece.chars().count())
}

/// `web_find`：给了 `file` 读文件；只给了 `url` 就现抓（本机抓，免费）。
pub(super) fn web_find(
    web: &Web,
    args: &Value,
    ctx: &Ctx<'_>,
    _: Option<&Quote>,
) -> Result<Output, ToolError> {
    let (article, _, _) = if args["file"].as_str().is_some_and(|f| !f.trim().is_empty()) {
        load_file(args, ctx)?
    } else {
        let Some(url) = url_arg(args)? else {
            return Err(ToolError::new(
                "bad_args",
                "要给 file 或 url。",
                "找存好的网页给 file；没存就给网页的 url，会现抓一次。",
                false,
            ));
        };
        let mode = mode_of(args);
        let got =
            fetch(&url, &web.fetch, web.transport.as_ref(), ctx.cancel).map_err(tool_error)?;
        ctx.cancel.check()?;
        let article = article_of(&got.bytes, got.content_type.as_deref(), &got.url, mode)?;
        (article, Some(got.url), mode)
    };
    let query: Query =
        serde_json::from_value(args.clone()).map_err(|_| ToolError::of("bad_args"))?;
    let under: Option<Vec<String>> = args["under"].as_array().map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    });
    let found: Found<crate::Loc> = find(&article, &query, under.as_deref()).map_err(tool_error)?;
    ctx.cancel.check()?;
    let mut text = match (found.total, found.hits.first().map(|h| h.how)) {
        (0, _) => "没有找到。换个说法、少几个词，或去掉 under 再试。".to_string(),
        (total, Some(Match::Words)) => {
            format!("整句没有找到，按词找到 {total} 处（几个词在同一段）：")
        }
        (total, _) => format!("找到 {total} 处："),
    };
    let flat = |s: &str| s.replace(['\n', '\t'], " ");
    for hit in &found.hits {
        text += &format!(
            "\n- {}：{}**{}**{}",
            hit.location.label(),
            flat(&hit.before),
            flat(&hit.matched),
            flat(&hit.after)
        );
    }
    if found.truncated {
        text += &format!(
            "\n\n一共 {} 处，只列了前 {} 处。",
            found.total,
            found.hits.len()
        );
    }
    if found.total > 0 {
        text += &format!("\n\n{DATA_NOTE}");
    }
    let data = serde_json::to_value(&found).map_err(|_| ToolError::of("internal"))?;
    Ok(Output {
        text,
        data,
        ..Output::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Author, Paper, PaperId};

    #[test]
    fn paper_lines_are_short() {
        let author = |n: usize| Author {
            name: format!("Author {n}"),
            affiliations: vec![format!("Lab {}", n % 7)],
            orcid: None,
        };
        let p = Paper {
            title: Some("T".into()),
            authors: (1..=12).map(author).collect(),
            journal: Some("Journal of Examples".into()),
            publisher: Some("Journal of Examples".into()),
            volume: Some("12".into()),
            issue: Some("3".into()),
            pages: Some("45–67".into()),
            published: Some("2024/05".into()),
            doi: Some("10.5555/x".into()),
            ids: vec![PaperId {
                scheme: "arxiv",
                value: "2401.00001".into(),
            }],
            pdf_url: Some("https://journal.example.org/x.pdf".into()),
            keywords: vec!["a".into(), "b".into()],
            ..Paper::default()
        };
        let lines = paper_lines(&p, None);
        assert_eq!(lines[0], "文献：");
        assert!(
            lines[1].starts_with("- 作者：Author 1; Author 2;")
                && lines[1].ends_with("Author 10 等 12 位")
        );
        assert!(lines[2].ends_with("等 7 个"), "{}", lines[2]);
        assert_eq!(
            lines[3], "- 出处：Journal of Examples，卷 12，期 3，页 45–67，发表 2024/05",
            "出版者和刊名一样就不重复"
        );
        assert_eq!(lines[4], "- 编号：DOI 10.5555/x · arXiv 2401.00001");
        assert_eq!(lines[5], "- PDF：https://journal.example.org/x.pdf");
        assert_eq!(lines[6], "- 关键词：a; b");
        assert!(paper_lines(&Paper::default(), None).is_empty());
        let dated = Paper {
            published: Some("2017/06/12".into()),
            ..Paper::default()
        };
        assert!(
            paper_lines(&dated, Some("2017/06/12")).is_empty(),
            "日期和上面一样就不再列"
        );
        assert_eq!(paper_lines(&dated, None)[1], "- 出处：发表 2017/06/12");
    }

    #[test]
    fn segments_break_at_paragraphs() {
        let md = "第一段第一段\n\n第二段第二段\n第二段第三行\n\n第三段";
        let (piece, end) = segment(md, 0, 18);
        assert_eq!((piece, end), ("第一段第一段\n\n第二段第二段\n", 15));
        let (piece, end) = segment(md, end, 100);
        assert_eq!((piece, end), ("第二段第三行\n\n第三段", md.chars().count()));
        assert_eq!(
            segment("一二三四五六", 0, 4),
            ("一二三四", 4),
            "没有换行就硬断"
        );
    }
}
