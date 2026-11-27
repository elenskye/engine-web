//! Tavily 的三个工具：`web_search`（搜索）、`web_crawl`（爬站，先报价、每次确认）、`web_map`（站内地图，同上）。

use super::{DATA_NOTE, Web, tavily_key, tool_error, usage, wrap};
use crate::check_url;
use crate::tavily::{self, Client, Crawl, Depth, Search};
use engine_core::Cancel;
use engine_core::call::{Ctx, Files, Output, Quote, ToolError};
use serde_json::{Value, json};

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn depth(v: &Value) -> Depth {
    Depth::parse(v.as_str().unwrap_or("basic"))
}

fn client<'a>(web: &'a Web, key: &'a str) -> Client<'a> {
    Client {
        key,
        base: &web.tavily_base,
        transport: web.transport.as_ref(),
    }
}

/// 截到 `max` 个字，截了就加「……」。
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}……", s.chars().take(max).collect::<String>())
    }
}

// ---------- web_search ----------

pub(super) fn quote_search(
    _: &Web,
    args: &Value,
    _: &dyn Files,
    _: &Cancel,
) -> Result<f64, ToolError> {
    Ok(tavily::quote::search(depth(&args["depth"])))
}

pub(super) fn web_search(
    web: &Web,
    args: &Value,
    ctx: &Ctx<'_>,
    quote: Option<&Quote>,
) -> Result<Output, ToolError> {
    let key = tavily_key(ctx)?;
    let s = Search {
        query: args["query"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string(),
        max_results: args["max_results"].as_u64().unwrap_or(5) as u32,
        depth: depth(&args["depth"]),
        topic: args["topic"].as_str().unwrap_or("general").to_string(),
        recency: args["recency"].as_str().map(str::to_string),
        domains: strings(&args["domains"]),
        exclude_domains: strings(&args["exclude_domains"]),
    };
    if s.query.is_empty() {
        return Err(ToolError::new(
            "bad_args",
            "query 是空的。",
            "写要搜的问题或关键词。",
            false,
        ));
    }
    if s.query.chars().count() > 400 {
        return Err(ToolError::new(
            "bad_args",
            "query 太长了（最多 400 字）。",
            "只留关键词，或拆成几次搜。",
            false,
        ));
    }
    let reply = client(web, &key)
        .search(&s, ctx.cancel)
        .map_err(tool_error)?;
    let used = usage(reply.credits, quote, ctx);
    let mut text = if reply.value.is_empty() {
        "没有搜到。换个说法、少几个词，或去掉 domains、recency 再试。".to_string()
    } else {
        format!("搜到 {} 条（Tavily）：", reply.value.len())
    };
    for (i, hit) in reply.value.iter().enumerate() {
        let title = if hit.title.is_empty() {
            "（没有标题）"
        } else {
            &hit.title
        };
        let date = hit
            .published
            .as_deref()
            .map(|d| format!(" · {d}"))
            .unwrap_or_default();
        text += &format!(
            "\n\n{}. {title}\n   {}{date}\n   {}",
            i + 1,
            hit.url,
            clip(&hit.content.replace('\n', " "), 300)
        );
    }
    if !reply.value.is_empty() {
        text += &format!("\n\n摘录是搜索服务给的，要读全文用 web_fetch。{DATA_NOTE}");
    }
    let data = json!({
        "query": s.query,
        "results": reply.value.iter().map(|h| {
            let mut r = json!({"title": h.title, "url": h.url, "content": h.content, "score": h.score});
            if let Some(d) = &h.published {
                r["published"] = json!(d);
            }
            r
        }).collect::<Vec<_>>(),
        "request_id": reply.request_id,
    });
    Ok(Output {
        text,
        data,
        usage: Some(used),
        ..Output::default()
    })
}

// ---------- web_crawl、web_map ----------

fn crawl_of(args: &Value) -> Result<Crawl, ToolError> {
    let url = check_url(args["url"].as_str().unwrap_or_default()).map_err(tool_error)?;
    Ok(Crawl {
        url: url.to_string(),
        max_depth: args["max_depth"].as_u64().unwrap_or(1) as u32,
        limit: args["limit"].as_u64().unwrap_or(10) as u32,
        instructions: args["instructions"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        select_paths: strings(&args["select_paths"]),
        extract_depth: depth(&args["extract_depth"]),
    })
}

pub(super) fn quote_crawl(
    _: &Web,
    args: &Value,
    _: &dyn Files,
    _: &Cancel,
) -> Result<f64, ToolError> {
    Ok(tavily::quote::crawl(&crawl_of(args)?))
}

pub(super) fn quote_map(
    _: &Web,
    args: &Value,
    _: &dyn Files,
    _: &Cancel,
) -> Result<f64, ToolError> {
    let c = crawl_of(args)?;
    Ok(tavily::quote::map(c.limit, c.instructions.is_some()))
}

pub(super) fn web_crawl(
    web: &Web,
    args: &Value,
    ctx: &Ctx<'_>,
    quote: Option<&Quote>,
) -> Result<Output, ToolError> {
    let c = crawl_of(args)?;
    let key = tavily_key(ctx)?;
    let reply = client(web, &key)
        .crawl(&c, ctx.cancel)
        .map_err(tool_error)?;
    let used = usage(reply.credits, quote, ctx);
    let pages: Vec<_> = reply
        .value
        .iter()
        .filter(|p| !p.content.is_empty())
        .collect();
    // 一共最多交 max_chars 字，平分给每页
    let max_chars = args["max_chars"].as_u64().unwrap_or(20_000) as usize;
    let each = (max_chars / pages.len().max(1)).max(200);
    let mut text = if pages.is_empty() {
        "没有爬到有正文的页。换个起点、放宽 select_paths，或用 web_map 先看看有哪些网址。"
            .to_string()
    } else {
        format!(
            "从 {} 爬到 {} 页（Tavily，每页最多 {each} 字；要读整页用 web_fetch）。{DATA_NOTE}",
            c.url,
            pages.len()
        )
    };
    let mut cut = false;
    for page in &pages {
        let body = clip(&page.content, each);
        cut |= body.len() < page.content.len();
        text += &format!("\n\n{}", wrap(Some(&page.url), &body));
    }
    let data = json!({
        "url": c.url,
        "pages": pages.len(),
        "results": pages.iter().map(|p| json!({"url": p.url, "chars": p.content.chars().count()})).collect::<Vec<_>>(),
        "clipped": cut,
        "request_id": reply.request_id,
    });
    Ok(Output {
        text,
        data,
        usage: Some(used),
        ..Output::default()
    })
}

pub(super) fn web_map(
    web: &Web,
    args: &Value,
    ctx: &Ctx<'_>,
    quote: Option<&Quote>,
) -> Result<Output, ToolError> {
    let c = crawl_of(args)?;
    let key = tavily_key(ctx)?;
    let reply = client(web, &key).map(&c, ctx.cancel).map_err(tool_error)?;
    let used = usage(reply.credits, quote, ctx);
    let mut text = if reply.value.is_empty() {
        "没有找到网址。换个起点，或去掉 instructions 再试。".to_string()
    } else {
        format!(
            "从 {} 找到 {} 个网址（Tavily）。要读哪页用 web_fetch：",
            c.url,
            reply.value.len()
        )
    };
    for url in &reply.value {
        text += &format!("\n- {url}");
    }
    let data = json!({
        "url": c.url,
        "count": reply.value.len(),
        "urls": reply.value,
        "request_id": reply.request_id,
    });
    Ok(Output {
        text,
        data,
        usage: Some(used),
        ..Output::default()
    })
}
