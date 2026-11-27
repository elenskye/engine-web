//! `web_fetch`：网址 → 正文。默认本机抓（免费）；`via: tavily` 改用 Tavily 取页（计费）；`via: auto` 先本机，
//! 取不到正文、被网站挡了、连不上时，App 给了 Tavily 密钥就改用 Tavily。PDF、图片这类原件作附件交回。

use super::extract::{article_of, mode_of, render};
use super::{Web, tavily_key, tool_error, usage};
use crate::tavily::{self, Client, Depth};
use crate::{Error, Mode, check_url, fetch, sniff};
use engine_core::Cancel;
use engine_core::call::{Attachment, AttachmentContent, Ctx, Files, Output, Quote, ToolError};
use serde_json::{Value, json};

/// 报价：本机抓不花钱；`tavily`、`auto` 按取一页的最坏情况报（basic 1 点、advanced 2 点）。
pub(super) fn quote(_: &Web, args: &Value, _: &dyn Files, _: &Cancel) -> Result<f64, ToolError> {
    Ok(match args["via"].as_str() {
        Some("tavily") | Some("auto") => tavily::quote::extract(1, depth(args)),
        _ => 0.0,
    })
}

fn depth(args: &Value) -> Depth {
    Depth::parse(args["depth"].as_str().unwrap_or("basic"))
}

pub(super) fn web_fetch(
    web: &Web,
    args: &Value,
    ctx: &Ctx<'_>,
    quote: Option<&Quote>,
) -> Result<Output, ToolError> {
    let raw = args["url"].as_str().unwrap_or_default();
    let url = check_url(raw).map_err(tool_error)?.to_string();
    let mode = mode_of(args);
    let via = args["via"].as_str().unwrap_or("local");
    if via == "tavily" {
        return by_tavily(web, &url, args, ctx, quote, None);
    }
    match local(web, &url, mode, args, ctx) {
        Ok(out) => Ok(out),
        Err(miss) if via == "auto" && miss.fallback && ctx.keys.get("tavily").is_some() => {
            by_tavily(web, &url, args, ctx, quote, Some(miss.why))
        }
        Err(miss) => Err(miss.error),
    }
}

/// 本机抓没成：错误、能不能改用 Tavily、给人看的原因。
struct Miss {
    error: ToolError,
    fallback: bool,
    why: String,
}

impl Miss {
    /// 能改用 Tavily 的：取不到正文、被网站挡了（401、403、429）、连不上、超时。内网、网址不对、跳到别的网站不换。
    fn of(e: Error) -> Self {
        let fallback = matches!(
            e,
            Error::Network
                | Error::Timeout
                | Error::NoArticle
                | Error::HttpStatus {
                    status: 401 | 403 | 429
                }
        );
        Self {
            why: e.to_string(),
            error: tool_error(e),
            fallback,
        }
    }

    fn tool(error: ToolError) -> Self {
        Self {
            fallback: error.kind == "no_article",
            why: "这个网页里没读到正文。".into(),
            error,
        }
    }
}

/// 本机抓。
fn local(web: &Web, url: &str, mode: Mode, args: &Value, ctx: &Ctx<'_>) -> Result<Output, Miss> {
    let got = fetch(url, &web.fetch, web.transport.as_ref(), ctx.cancel).map_err(Miss::of)?;
    ctx.cancel.check().map_err(|e| Miss::tool(e.into()))?;
    // PDF、图片这类：不读，原件作附件交回，说明交给哪个工具
    if let Err(what) = sniff(&got.bytes, got.content_type.as_deref(), &got.url) {
        let e = tool_error(Error::NotWeb(what));
        let name = file_name(&got.url, what.key());
        let text = format!(
            "{}原件（{}）附在结果里：{}\n网址：{}",
            e.message,
            size(got.bytes.len()),
            e.hint,
            got.url
        );
        let data = json!({
            "url": got.url, "via": "local", "status": got.status, "not_web": what.key(),
            "bytes": got.bytes.len(), "body_truncated": got.truncated, "redirects": got.redirects,
            "attachment": name,
        });
        return Ok(Output {
            text,
            data,
            attachments: vec![Attachment {
                filename: name,
                mime_type: got
                    .content_type
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".into()),
                content: AttachmentContent::Bytes(got.bytes),
            }],
            ..Output::default()
        });
    }
    let article = article_of(&got.bytes, got.content_type.as_deref(), &got.url, mode)
        .map_err(|e| Miss::tool(fill_no_article(e)))?;
    let mut head = vec![format!("网址：{}", got.url)];
    if !got.redirects.is_empty() {
        head.push(format!("跳转：{} → {}", got.redirects.join(" → "), got.url));
    }
    if got.truncated {
        head.push(format!(
            "网页超过 {}，只读了前面一部分。",
            size(web.fetch.max_bytes.min(crate::LIMITS.max_bytes) as usize)
        ));
    }
    let more = json!({
        "via": "local", "status": got.status, "redirects": got.redirects, "body_truncated": got.truncated,
    });
    render(&article, Some(&got.url), head, mode, args, more).map_err(Miss::tool)
}

/// 本机抓到了但正文是空的：说明里提一句可以换 Tavily。
fn fill_no_article(e: ToolError) -> ToolError {
    if e.kind != "no_article" {
        return e;
    }
    ToolError::new(
        "no_article",
        "这个网页里没读到正文。",
        "多半要开 JavaScript 才有正文：可以换 via: tavily（花 Tavily 点数）再试，或换 mode: full，不行就告诉用户。",
        false,
    )
}

/// 用 Tavily 取页。`from` 是本机抓为什么没成（`auto` 时）。
fn by_tavily(
    web: &Web,
    url: &str,
    args: &Value,
    ctx: &Ctx<'_>,
    quote: Option<&Quote>,
    from: Option<String>,
) -> Result<Output, ToolError> {
    let key = tavily_key(ctx)?;
    let client = Client {
        key: key.as_str(),
        base: &web.tavily_base,
        transport: web.transport.as_ref(),
    };
    let reply = client
        .extract(&[url.to_string()], depth(args), ctx.cancel)
        .map_err(tool_error)?;
    let used = usage(reply.credits, quote, ctx);
    let Some(page) = reply
        .value
        .pages
        .into_iter()
        .find(|p| !p.content.trim().is_empty())
    else {
        return Err(ToolError::new(
            "no_article",
            "Tavily 也没取到这个网页的正文。",
            "网页可能要登录、有验证码，或正文是图片：告诉用户这一页读不到。",
            false,
        )
        .with_usage(Some(used)));
    };
    let mode = mode_of(args);
    let article = article_of(
        page.content.as_bytes(),
        Some("text/markdown"),
        &page.url,
        mode,
    )
    .map_err(|e| e.with_usage(Some(used.clone())))?;
    let mut head = vec![
        format!("网址：{}", page.url),
        "来自：Tavily 取页（本机没有直接抓）".to_string(),
    ];
    if let Some(why) = &from {
        head.push(format!("本机抓没成：{why}"));
    }
    let mut more = json!({"via": "tavily"});
    if let Some(why) = &from {
        more["fallback_from"] = json!(why);
    }
    let mut out = render(&article, Some(&page.url), head, mode, args, more)
        .map_err(|e| e.with_usage(Some(used.clone())))?;
    out.usage = Some(used);
    Ok(out)
}

/// 附件的文件名：网址最后一段，没有就用类型。
fn file_name(url: &str, kind: &str) -> String {
    let last = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim();
    let safe: String = last
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '\0'))
        .take(120)
        .collect();
    if safe.is_empty() || safe.starts_with('.') {
        format!("download.{kind}")
    } else {
        safe
    }
}

fn size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_names_come_from_the_url() {
        assert_eq!(
            file_name("https://x.example/papers/a.pdf?dl=1", "pdf"),
            "a.pdf"
        );
        assert_eq!(file_name("https://x.example/", "pdf"), "download.pdf");
        assert_eq!(file_name("https://x.example/..", "image"), "download.image");
        assert_eq!(size(2_202_009), "2.1 MB");
        assert_eq!(size(10), "1 KB");
    }
}
