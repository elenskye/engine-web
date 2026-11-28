//! 认出挡程序的验证页（站主 2026-11-28 定，`decisions/0008`）：Cloudflare 的「Just a moment...」、
//! PubMed 的「Cookies must be enabled」、公众号的「环境异常」这类。认出来工具就报 `bot_check`，让模型改用 Tavily，
//! 不把那几句话当正文交出去。
//!
//! 只看正文很短的页（不算空白不到 1500 字）：长文章里提到这些字、普通网页里 Cloudflare 塞的脚本都不算。
//! 先看网页里各家验证页特有的写法（脚本变量、路径），再看标题和正文里的几句话。

/// 正文比这长就不是验证页。
const MAX_CHARS: usize = 1500;

/// 原 HTML 里各家验证页特有的写法 → 哪一家。
const SIGNS: &[(&str, &str)] = &[
    ("window._cf_chl_opt", "Cloudflare"),
    ("/cdn-cgi/challenge-platform/h/", "Cloudflare"),
    ("cf-browser-verification", "Cloudflare"),
    ("/_fs-ch-", "Fastly"),
    ("_Incapsula_Resource", "Imperva"),
    ("captcha-delivery.com", "DataDome"),
    ("px-captcha", "PerimeterX"),
    ("awsWafCookieDomainList", "AWS WAF"),
    ("gokuProps", "AWS WAF"),
    ("sucuri_cloudproxy_js", "Sucuri"),
    ("errors.edgesuite.net", "Akamai"),
    ("within.website/x/cmd/anubis", "Anubis"),
    ("anubis_challenge", "Anubis"),
    ("Vercel Security Checkpoint", "Vercel"),
    ("ddos-guard", "DDoS-Guard"),
    ("/.stile/challenge/", "unknown"),
    // PubMed：算工作量证明的脚本，「Cookies must be enabled」藏着、脚本失败才显示
    ("const challengeId = ", "unknown"),
    ("secitptpage/verify", "微信"),
];

/// 标题（小写，去掉首尾空白）就是这几句的。
const TITLES: &[&str] = &[
    "just a moment...",
    "attention required! | cloudflare",
    "client challenge",
    "checking your browser",
    "security check",
    "verifying you are human",
    "human verification",
    "are you a robot?",
    "pardon our interruption",
    "robot check",
    "ddos-guard",
    "安全验证",
    "人机验证",
    "环境异常",
];

/// 正文里（小写）有这几句的。
const PHRASES: &[&str] = &[
    "checking your browser",
    "verifying you are human",
    "verify you are human",
    "verifying your connection",
    "enable javascript and cookies to continue",
    "cookies must be enabled",
    "please complete the security check",
    "pardon our interruption",
    "环境异常",
    "完成验证后即可继续访问",
    "请完成安全验证",
];

/// 是验证页就交哪一家（认不出是哪家的交 `unknown`）。`html` 是原来的网页，`text` 是取出来的正文。
pub(super) fn detect(html: &str, title: Option<&str>, text: &str) -> Option<&'static str> {
    if text.chars().filter(|c| !c.is_whitespace()).count() > MAX_CHARS {
        return None;
    }
    if let Some((_, who)) = SIGNS.iter().find(|(sign, _)| html.contains(sign)) {
        return Some(who);
    }
    let title = title.unwrap_or_default().trim().to_lowercase();
    let text = text.to_lowercase();
    (TITLES.contains(&title.as_str()) || PHRASES.iter().any(|p| text.contains(p)))
        .then_some("unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_pages_are_recognised() {
        let cf = r#"<html><head><title>Just a moment...</title></head><body><script>window._cf_chl_opt={cType:'managed'};</script><noscript>Enable JavaScript and cookies to continue</noscript></body></html>"#;
        assert_eq!(
            detect(
                cf,
                Some("Just a moment..."),
                "Enable JavaScript and cookies to continue"
            ),
            Some("Cloudflare")
        );
        assert_eq!(
            detect(
                "<p>Cookies must be enabled</p>",
                Some("pubmed.ncbi.nlm.nih.gov"),
                "Cookies must be enabled Enable cookies for pubmed.ncbi.nlm.nih.gov and reload this page to continue."
            ),
            Some("unknown")
        );
        assert_eq!(
            detect(
                "<title>Client Challenge</title>",
                Some("Client Challenge"),
                "JavaScript is disabled in your browser."
            ),
            Some("unknown")
        );
        assert_eq!(
            detect(
                r#"<script>var PAGE_MID='mmbizwap:secitptpage/verify.html';</script>"#,
                None,
                "视频 小程序 赞"
            ),
            Some("微信")
        );
    }

    #[test]
    fn real_pages_are_not_challenges() {
        // Cloudflare 给普通网页也塞脚本；正文长的不算
        let normal =
            r#"<script src="/cdn-cgi/challenge-platform/scripts/jsd/main.js"></script><p>正文</p>"#;
        assert_eq!(
            detect(normal, Some("一篇文章"), "正文很短，但没有验证的话。"),
            None
        );
        let long = format!(
            "{} Checking your browser before accessing is what Cloudflare does.",
            "长文章。".repeat(500)
        );
        assert_eq!(
            detect(
                "<title>Just a moment...</title>",
                Some("Just a moment..."),
                &long
            ),
            None
        );
        // 单页应用要开 JavaScript 不是挡程序（那是 no_article）
        assert_eq!(
            detect(
                "<noscript>You need to enable JavaScript to run this app.</noscript>",
                Some("App"),
                "You need to enable JavaScript to run this app."
            ),
            None
        );
    }
}
