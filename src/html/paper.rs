//! 文献信息（`docs/decisions/0008`）：`citation_*`（Highwire Press，Google Scholar 收录用的那套）为主，
//! PRISM、Dublin Core 补没写的。只读 `<meta>`，照原样（去掉标签和多余空白），不猜、不补。

use crate::{Author, Paper, PaperId};
use dom_query::{Document, Selection};
use regex::Regex;
use std::sync::OnceLock;
use url::Url;

/// 网页里的 `<meta name|property=… content=…>`，名字转小写，照出现的顺序。
fn metas(doc: &Document) -> Vec<(String, String)> {
    doc.select("meta")
        .nodes()
        .iter()
        .filter_map(|m| {
            let name = m.attr("name").or_else(|| m.attr("property"))?;
            let content = m.attr("content")?;
            Some((name.trim().to_ascii_lowercase(), clean(&content)))
        })
        .collect()
}

/// 去掉夹在值里的标签（`<i>`、`<jats:p>`；`p < 0.05` 这种不算标签），空白收成一个。
fn clean(value: &str) -> String {
    static TAG: OnceLock<Regex> = OnceLock::new();
    let tag = TAG.get_or_init(|| Regex::new(r"</?[A-Za-z][A-Za-z0-9:-]*(?:\s[^<>]*)?/?>").unwrap());
    tag.replace_all(value, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

struct Metas(Vec<(String, String)>);

impl Metas {
    /// 第一个有值的。
    fn first(&self, names: &[&str]) -> Option<String> {
        names.iter().find_map(|n| {
            self.0
                .iter()
                .find(|(k, v)| k == n && !v.is_empty())
                .map(|(_, v)| v.clone())
        })
    }

    /// 这个名字的所有值。
    fn all(&self, name: &str) -> Vec<String> {
        self.0
            .iter()
            .filter(|(k, v)| k == name && !v.is_empty())
            .map(|(_, v)| v.clone())
            .collect()
    }

    fn has(&self, name: &str) -> bool {
        self.first(&[name]).is_some()
    }
}

/// 读文献信息。没有 `citation_title`、也没有 PRISM 的刊名加 DOI 时交 `None`（只有 Dublin Core 的普通网页不算）。
pub(super) fn read(doc: &Document, base: Option<&Url>) -> Option<Paper> {
    let m = Metas(metas(doc));
    let doi = m
        .first(&["citation_doi", "prism.doi", "bepress_citation_doi"])
        .or_else(|| {
            ["dc.identifier", "dcterms.identifier"]
                .iter()
                .flat_map(|n| m.all(n))
                .find(|v| looks_like_doi(v))
        })
        .and_then(|d| normalize_doi(&d));
    let scholarly = m.has("citation_title") || (m.has("prism.publicationname") && doi.is_some());
    if !scholarly {
        return None;
    }
    let first_page = m.first(&["citation_firstpage", "prism.startingpage"]);
    let last_page = m.first(&["citation_lastpage", "prism.endingpage"]);
    let pages = match (first_page, last_page) {
        (Some(f), Some(l)) if f != l => Some(format!("{f}–{l}")),
        (Some(f), _) => Some(f),
        (None, _) => None,
    };
    let mut ids = Vec::new();
    for (scheme, names) in [
        ("arxiv", &["citation_arxiv_id"][..]),
        ("pmid", &["citation_pmid"]),
        ("pmcid", &["citation_pmcid"]),
        ("issn", &["citation_issn", "prism.issn", "prism.eissn"]),
        ("isbn", &["citation_isbn"]),
    ] {
        for name in names {
            for value in m.all(name) {
                if !ids
                    .iter()
                    .any(|i: &PaperId| i.scheme == scheme && i.value == value)
                {
                    ids.push(PaperId { scheme, value });
                }
            }
        }
    }
    let paper = Paper {
        title: m.first(&["citation_title", "dc.title", "dcterms.title"]),
        authors: authors(&m),
        abstract_text: m.first(&["citation_abstract", "dcterms.abstract"]),
        keywords: keywords(&m),
        journal: m.first(&[
            "citation_journal_title",
            "citation_conference_title",
            "prism.publicationname",
            "citation_series_title",
            "citation_technical_report_institution",
            "citation_dissertation_institution",
        ]),
        publisher: m.first(&["citation_publisher", "dc.publisher", "dcterms.publisher"]),
        volume: m.first(&["citation_volume", "prism.volume"]),
        issue: m.first(&["citation_issue", "prism.number"]),
        pages,
        number: m.first(&["citation_technical_report_number"]),
        published: m.first(&[
            "citation_publication_date",
            "citation_date",
            "prism.publicationdate",
            "citation_online_date",
            "prism.coverdate",
            "dc.date",
            "dcterms.date",
            "dcterms.issued",
            "citation_year",
        ]),
        doi,
        pdf_url: m
            .first(&["citation_pdf_url"])
            .map(|u| super::markdown::absolute(&u, base)),
        ids,
    };
    Some(paper)
}

/// 作者：`citation_author` 一个一位，后面跟着的 `citation_author_institution` 是这位的机构；
/// 没有就看 `citation_authors`（分号隔开）、`dc.creator`（一个一位）。
fn authors(m: &Metas) -> Vec<Author> {
    let mut out: Vec<Author> = Vec::new();
    for (name, value) in &m.0 {
        if value.is_empty() {
            continue;
        }
        match name.as_str() {
            "citation_author" => out.push(Author {
                name: value.clone(),
                affiliations: Vec::new(),
                orcid: None,
            }),
            "citation_author_institution" | "citation_author_affiliation" => {
                if let Some(last) = out.last_mut() {
                    if !last.affiliations.contains(value) {
                        last.affiliations.push(value.clone());
                    }
                }
            }
            "citation_author_orcid" => {
                if let Some(last) = out.last_mut() {
                    let id = value
                        .trim_start_matches("https://orcid.org/")
                        .trim_start_matches("http://orcid.org/");
                    last.orcid = Some(id.to_string());
                }
            }
            _ => {}
        }
    }
    let plain = |names: Vec<String>| {
        names
            .into_iter()
            .map(|name| Author {
                name,
                affiliations: Vec::new(),
                orcid: None,
            })
            .collect::<Vec<_>>()
    };
    if out.is_empty() {
        if let Some(list) = m.first(&["citation_authors"]) {
            out = plain(split_list(&list, ';'));
        }
    }
    if out.is_empty() {
        let creators = [m.all("dc.creator"), m.all("dcterms.creator")].concat();
        out = plain(creators);
    }
    out
}

/// 关键词：`citation_keywords`（可以写几次，一次里用分号或逗号隔开）、`prism.keyword`；都没有才用 `dc.subject`。
fn keywords(m: &Metas) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for value in m.all("citation_keywords") {
        let sep = if value.contains(';') { ';' } else { ',' };
        found.extend(split_list(&value, sep));
    }
    found.extend(m.all("prism.keyword"));
    if found.is_empty() {
        found = m.all("dc.subject");
    }
    let mut words: Vec<String> = Vec::new();
    for w in found {
        if !w.is_empty() && !words.contains(&w) {
            words.push(w);
        }
    }
    words
}

fn split_list(value: &str, sep: char) -> Vec<String> {
    value
        .split(sep)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn looks_like_doi(value: &str) -> bool {
    normalize_doi(value).is_some()
}

/// `doi:10.1/x`、`https://doi.org/10.1/x`、`10.1/x` → `10.1/x`；不是 DOI 交 `None`。
fn normalize_doi(value: &str) -> Option<String> {
    let v = value.trim();
    let lower = v.to_ascii_lowercase();
    let start = [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "doi:",
    ]
    .iter()
    .find(|p| lower.starts_with(*p))
    .map_or(0, |p| p.len());
    let doi = v[start..].trim();
    (doi.starts_with("10.") && doi.contains('/')).then(|| doi.to_string())
}

/// 文献页的参考文献后面常跟一串查找链接（Google Scholar、Nature 的 CAS）：不带内容，长页上一两百个。
/// 去掉这些链接；只剩它的列表项一起去掉。参考文献本身、DOI 和 PubMed 的链接留着。交回去没去掉东西。
pub(super) fn drop_lookup_links(doc: &Document) -> bool {
    let links = doc.select(r#"a[href*="scholar.google."], a[href*="/cas-redirect/"]"#);
    if links.nodes().is_empty() {
        return false;
    }
    for a in links.nodes() {
        let parent = a.parent();
        a.remove_from_parent();
        if let Some(p) = parent {
            let li = Selection::from(p);
            if li.is("li") && p.text().trim().is_empty() {
                p.remove_from_parent();
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paper(head: &str) -> Option<Paper> {
        read(
            &Document::from(format!("<html><head>{head}</head><body></body></html>").as_str()),
            Url::parse("https://journal.example.org/a/1").ok().as_ref(),
        )
    }

    #[test]
    fn citation_tags_make_a_paper() {
        let p = paper(
            r#"<meta name="citation_title" content="On  <i>Small</i> Samples">
            <meta name="citation_author" content="Moss, Ada">
            <meta name="citation_author_institution" content="Example University">
            <meta name="citation_author_institution" content="Example Institute">
            <meta name="citation_author_orcid" content="https://orcid.org/0000-0002-1825-0097">
            <meta name="citation_author" content="Lin, Wei">
            <meta name="citation_journal_title" content="Journal of Examples">
            <meta name="citation_volume" content="12"><meta name="citation_issue" content="3">
            <meta name="citation_firstpage" content="45"><meta name="citation_lastpage" content="67">
            <meta name="citation_publication_date" content="2024/05/01">
            <meta name="citation_doi" content="doi:10.5555/example.2024.12">
            <meta name="citation_pdf_url" content="/a/1.pdf">
            <meta name="citation_keywords" content="sampling; power, effect size; bias">
            <meta name="citation_issn" content="1234-5678"><meta name="prism.issn" content="1234-5678">
            <meta name="citation_abstract" content="&lt;p&gt;We test p &lt; 0.05 claims.&lt;/p&gt;">"#,
        )
        .unwrap();
        assert_eq!(p.title.as_deref(), Some("On Small Samples"));
        assert_eq!(p.authors.len(), 2);
        assert_eq!(
            p.authors[0].affiliations,
            ["Example University", "Example Institute"]
        );
        assert_eq!(p.authors[0].orcid.as_deref(), Some("0000-0002-1825-0097"));
        assert!(p.authors[1].affiliations.is_empty());
        assert_eq!(p.pages.as_deref(), Some("45–67"));
        assert_eq!(p.doi.as_deref(), Some("10.5555/example.2024.12"));
        assert_eq!(
            p.pdf_url.as_deref(),
            Some("https://journal.example.org/a/1.pdf")
        );
        assert_eq!(p.keywords, ["sampling", "power, effect size", "bias"]);
        assert_eq!(p.abstract_text.as_deref(), Some("We test p < 0.05 claims."));
        assert_eq!(p.ids.len(), 1, "同一个 ISSN 只记一次");
    }

    #[test]
    fn prism_with_doi_is_enough_but_dublin_core_alone_is_not() {
        let p = paper(
            r#"<meta name="prism.publicationName" content="Example Letters">
            <meta name="dc.identifier" content="doi:10.5555/el.7">
            <meta name="dc.title" content="A Short Note">
            <meta name="dc.creator" content="Ada Moss"><meta name="dc.creator" content="Wei Lin">
            <meta name="prism.startingPage" content="e7">
            <meta name="dc.subject" content="Statistics">"#,
        )
        .unwrap();
        assert_eq!(p.title.as_deref(), Some("A Short Note"));
        assert_eq!(p.journal.as_deref(), Some("Example Letters"));
        assert_eq!(p.doi.as_deref(), Some("10.5555/el.7"));
        assert_eq!(p.pages.as_deref(), Some("e7"));
        let names: Vec<&str> = p.authors.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Ada Moss", "Wei Lin"]);
        assert_eq!(p.keywords, ["Statistics"]);
        assert!(
            paper(r#"<meta name="DC.title" content="市政府公告"><meta name="DC.creator" content="办公厅"><meta name="DC.date" content="2026-01-01">"#)
                .is_none(),
            "只有 Dublin Core 的普通网页不算"
        );
        assert!(
            paper(r#"<meta name="citation_doi" content="10.5555/x">"#).is_none(),
            "只有 DOI 不算"
        );
    }

    #[test]
    fn semicolon_author_lists_and_report_numbers() {
        let p = paper(
            r#"<meta name="citation_title" content="Working Paper">
            <meta name="citation_authors" content="Ada Moss; Wei Lin;">
            <meta name="citation_technical_report_institution" content="Example Bureau">
            <meta name="citation_technical_report_number" content="w123">"#,
        )
        .unwrap();
        let names: Vec<&str> = p.authors.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Ada Moss", "Wei Lin"]);
        assert_eq!(p.journal.as_deref(), Some("Example Bureau"));
        assert_eq!(p.number.as_deref(), Some("w123"));
    }

    #[test]
    fn dois_are_normalized() {
        assert_eq!(
            normalize_doi("https://doi.org/10.1/ABC").as_deref(),
            Some("10.1/ABC")
        );
        assert_eq!(normalize_doi("DOI:10.1/x").as_deref(), Some("10.1/x"));
        assert_eq!(normalize_doi("https://example.org/10.1/x"), None);
        assert_eq!(normalize_doi("10.1"), None);
    }

    #[test]
    fn lookup_links_go_but_references_stay() {
        let doc = Document::from(
            r#"<ol><li>Moss A. On samples. <a href="https://doi.org/10.1/x">Article</a> <a href="http://scholar.google.com/scholar_lookup?title=x">Google Scholar</a></li></ol>
            <ul class="reflinks"><li><a href="https://doi.org/10.1/y">View Article</a></li><li><a href="http://scholar.google.com/scholar?q=y">Google Scholar</a></li></ul>
            <p><a href="https://www.nature.com/articles/cas-redirect/1:CAS:528">CAS</a></p>"#,
        );
        assert!(drop_lookup_links(&doc));
        let html = doc.html().to_string();
        assert!(html.contains("Moss A. On samples.") && html.contains("View Article"));
        assert!(!html.contains("Google Scholar") && !html.contains("CAS<"));
        assert_eq!(
            doc.select("ul.reflinks li").length(),
            1,
            "空了的列表项一起去掉"
        );
        assert!(!drop_lookup_links(&Document::from("<p>没有</p>")));
    }
}
