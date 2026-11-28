//! GitHub（`github.com/<owner>/<repo>…`）：仓库首页、目录页、文件页的 README / Markdown，issue，代码文件。
//!
//! - README、Markdown 文件：正文是渲染好的 `article.markdown-body`。
//! - issue：网页里的 JSON 数据块（`react-app.embeddedData`）有标题、状态、标签、提问和回复（`bodyHTML`），从那里拼。
//!   长的 issue 网页里只放前后几条，中间的写明缺几条、给出 GitHub 接口的网址（站主 2026-11-28 定：不另调接口）。
//! - 代码文件：数据块里的 `rawLines` 拼成一段代码。
//! - 标题：「仓库 / 文件」（`BurntSushi/ripgrep / GUIDE.md`）；issue 是「标题 · 仓库#号」，作者、日期是提问的人和时间。

use super::{BLANK, Site, SiteMeta, escape};
use dom_query::Document;
use serde_json::Value;
use url::Url;

pub(super) const SITE: Site = Site {
    key: "github",
    matches,
    root: &["article.markdown-body"],
    drop: &["a.anchor"],
    build: Some(build),
    meta: Some(meta),
    ..BLANK
};

/// 不是仓库的一级路径。
const RESERVED: &[&str] = &[
    "about",
    "apps",
    "codespaces",
    "collections",
    "enterprise",
    "explore",
    "features",
    "login",
    "marketplace",
    "new",
    "notifications",
    "orgs",
    "pricing",
    "pulls",
    "search",
    "settings",
    "sponsors",
    "topics",
    "trending",
    "users",
];

fn matches(u: &Url) -> bool {
    let segments = segments(u);
    u.host_str() == Some("github.com")
        && segments.len() >= 2
        && !RESERVED.contains(&segments[0].as_str())
}

fn segments(u: &Url) -> Vec<String> {
    u.path_segments()
        .map(|s| s.filter(|p| !p.is_empty()).map(str::to_string).collect())
        .unwrap_or_default()
}

/// 网页里的 JSON 数据块。
fn embedded(doc: &Document) -> Option<Value> {
    let script = doc.select(r#"script[data-target="react-app.embeddedData"]"#);
    let text = script.nodes().first()?.text();
    serde_json::from_str(&text).ok()
}

fn issue_of(data: &Value) -> Option<&Value> {
    let issue = &data["payload"]["issueViewerRoute"]["data"]["repository"]["issue"];
    issue.is_object().then_some(issue)
}

fn build(doc: &Document) -> Option<String> {
    let data = embedded(doc)?;
    if let Some(issue) = issue_of(&data) {
        return Some(issue_html(issue));
    }
    // 代码文件：渲染好的 Markdown（`richText`）照 `root` 读，没有的才拼代码
    let blob = &data["payload"]["codeViewBlobLayoutRoute.StyledBlob"];
    let lines = blob["rawLines"].as_array()?;
    if data["payload"]["codeViewBlobRoute"]["richText"].is_string()
        || doc.select("article.markdown-body").exists()
    {
        return None;
    }
    let code: Vec<&str> = lines.iter().filter_map(Value::as_str).collect();
    let language = data["payload"]["codeViewBlobLayoutRoute"]["blob"]["language"]
        .as_str()
        .map(|l| format!(r#" class="language-{}""#, escape(&l.to_lowercase())))
        .unwrap_or_default();
    Some(format!(
        "<pre><code{language}>{}</code></pre>",
        escape(&code.join("\n"))
    ))
}

fn meta(doc: &Document, url: &Url) -> SiteMeta {
    let segments = segments(url);
    let repo = format!("{}/{}", segments[0], segments[1]);
    let site = Some("GitHub".to_string());
    let Some(data) = embedded(doc) else {
        return SiteMeta {
            site,
            ..SiteMeta::default()
        };
    };
    if let Some(issue) = issue_of(&data) {
        let name = issue["repository"]["nameWithOwner"]
            .as_str()
            .unwrap_or(&repo);
        let number = issue["url"]
            .as_str()
            .and_then(|u| u.rsplit('/').next())
            .unwrap_or_default();
        return SiteMeta {
            title: issue["title"]
                .as_str()
                .map(|t| format!("{t} · {name}#{number}")),
            byline: issue["author"]["login"].as_str().map(str::to_string),
            published: issue["createdAt"].as_str().map(str::to_string),
            site,
        };
    }
    // 只管仓库首页、文件页、目录页；提交记录这类照网页自己的标题
    if segments.len() > 2 && !matches!(segments[2].as_str(), "blob" | "tree") {
        return SiteMeta {
            site,
            ..SiteMeta::default()
        };
    }
    let payload = &data["payload"];
    // 文件页、目录页：路径；仓库首页：README 的文件名
    let path = payload["codeViewBlobLayoutRoute"]["path"]
        .as_str()
        .or_else(|| payload["codeViewLayoutRoute"]["path"].as_str())
        .filter(|p| !p.is_empty() && *p != "/")
        .or_else(|| {
            payload["codeViewRepoRoute"]["overview"]["overviewFiles"][0]["displayName"].as_str()
        });
    SiteMeta {
        title: Some(match path {
            Some(p) => format!("{repo} / {p}"),
            None => repo,
        }),
        site,
        ..SiteMeta::default()
    }
}

/// issue → HTML：状态那几行、提问、回复和关闭 / 重开，中间没放进网页的写明缺几条。
fn issue_html(issue: &Value) -> String {
    let opener = login(&issue["author"]);
    let mut out = String::from("<ul>");
    out += &format!("<li>State: {}</li>", escape(&state(issue)));
    let labels: Vec<&str> = list(&issue["labels"]["edges"])
        .iter()
        .filter_map(|e| e["node"]["name"].as_str())
        .collect();
    if !labels.is_empty() {
        out += &format!("<li>Labels: {}</li>", escape(&labels.join(", ")));
    }
    if let Some(m) = issue["milestone"]["title"].as_str() {
        out += &format!("<li>Milestone: {}</li>", escape(m));
    }
    let pulls: Vec<String> = list(&issue["linkedPullRequests"]["nodes"])
        .iter()
        .filter_map(|p| {
            let number = p["number"].as_u64()?;
            let url = p["url"].as_str()?;
            let title = p["title"].as_str().unwrap_or_default();
            let state = p["state"].as_str().unwrap_or_default().to_lowercase();
            Some(format!(
                r#"<a href="{}">#{number}</a> {} ({})"#,
                escape(url),
                escape(title),
                escape(&state)
            ))
        })
        .collect();
    if !pulls.is_empty() {
        out += &format!("<li>Pull requests: {}</li>", pulls.join("; "));
    }
    out += "</ul>";
    out += &heading(&opener, &issue["createdAt"], &[]);
    out += issue["bodyHTML"].as_str().unwrap_or_default();

    let front = list(&issue["timelineItems"]["edges"]);
    let back = list(&issue["backTimelineItems"]["edges"]);
    let total = issue["timelineItems"]["totalCount"].as_u64().unwrap_or(0) as usize;
    let mut seen = std::collections::HashSet::new();
    let mut shown = 0;
    for (i, edge) in front.iter().chain(back.iter()).enumerate() {
        if i == front.len() {
            let missing = total.saturating_sub(front.len() + back.len());
            if missing > 0 {
                out += &gap(issue, missing);
            }
        }
        let node = &edge["node"];
        if !seen.insert(node["id"].as_str().unwrap_or_default().to_string()) {
            continue;
        }
        shown += 1;
        out += &item(node, &opener);
    }
    if front.is_empty() && back.is_empty() && total > shown {
        out += &gap(issue, total);
    }
    out
}

fn list(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn login(actor: &Value) -> String {
    actor["login"].as_str().unwrap_or("ghost").to_string()
}

fn state(issue: &Value) -> String {
    match (issue["state"].as_str(), issue["stateReason"].as_str()) {
        (Some("OPEN"), _) => "Open".into(),
        (Some("CLOSED"), Some("COMPLETED")) => "Closed as completed".into(),
        (Some("CLOSED"), Some("NOT_PLANNED")) => "Closed as not planned".into(),
        (Some("CLOSED"), Some("DUPLICATE")) => "Closed as duplicate".into(),
        (Some("CLOSED"), _) => "Closed".into(),
        (Some(s), _) => s.to_string(),
        _ => "Unknown".into(),
    }
}

/// `2016-10-13T21:31:20Z` → `2016-10-13 21:31 UTC`。
fn time(v: &Value) -> String {
    let t = v.as_str().unwrap_or_default();
    match (t.get(..10), t.get(11..16)) {
        (Some(day), Some(clock)) if t.ends_with('Z') => format!("{day} {clock} UTC"),
        _ => t.to_string(),
    }
}

/// 每条回复的小标题：`@作者 (Owner, Author) · 时间`。
fn heading(who: &str, at: &Value, badges: &[&str]) -> String {
    let badges = if badges.is_empty() {
        String::new()
    } else {
        format!(" ({})", badges.join(", "))
    };
    format!(
        "<h2>@{}{} · {}</h2>",
        escape(who),
        escape(&badges),
        escape(&time(at))
    )
}

fn item(node: &Value, opener: &str) -> String {
    match node["__typename"].as_str() {
        Some("IssueComment") => {
            let who = login(&node["author"]);
            let mut badges = Vec::new();
            match node["authorAssociation"].as_str() {
                Some("OWNER") => badges.push("Owner"),
                Some("MEMBER") => badges.push("Member"),
                Some("COLLABORATOR") => badges.push("Collaborator"),
                _ => {}
            }
            if who == opener {
                badges.push("Author");
            }
            let mut out = heading(&who, &node["createdAt"], &badges);
            if node["isMinimized"].as_bool() == Some(true) {
                let reason = node["minimizedReason"]
                    .as_str()
                    .unwrap_or("hidden")
                    .to_lowercase()
                    .replace('_', " ");
                out += &format!("<p><i>Hidden by GitHub: {}</i></p>", escape(&reason));
            } else {
                out += node["bodyHTML"].as_str().unwrap_or_default();
            }
            out
        }
        Some("ClosedEvent") => {
            let reason = match node["stateReason"].as_str() {
                Some("COMPLETED") => " as completed",
                Some("NOT_PLANNED") => " as not planned",
                Some("DUPLICATE") => " as duplicate",
                _ => "",
            };
            let by = match (
                node["closer"]["number"].as_u64(),
                node["closer"]["url"].as_str(),
            ) {
                (Some(n), Some(url)) => format!(r#" in <a href="{}">#{n}</a>"#, escape(url)),
                _ => String::new(),
            };
            format!(
                "<p><i>@{} closed this{reason}{by} · {}</i></p>",
                escape(&login(&node["actor"])),
                escape(&time(&node["createdAt"]))
            )
        }
        Some("ReopenedEvent") => format!(
            "<p><i>@{} reopened this · {}</i></p>",
            escape(&login(&node["actor"])),
            escape(&time(&node["createdAt"]))
        ),
        _ => String::new(),
    }
}

/// 网页里没放的那几条：写明几条，给出接口的网址（全部回复，不含事件）。
fn gap(issue: &Value, missing: usize) -> String {
    let api = issue["url"].as_str().and_then(|u| {
        let rest = u.strip_prefix("https://github.com/")?;
        Some(format!(
            "https://api.github.com/repos/{rest}/comments?per_page=100"
        ))
    });
    let link = api
        .map(|a| format!(r#" All comments: <a href="{}">GitHub API</a>"#, escape(&a)))
        .unwrap_or_default();
    format!(
        "<p><i>{missing} more items (comments and events) are not in this page; GitHub loads them on demand.{link}</i></p>"
    )
}
