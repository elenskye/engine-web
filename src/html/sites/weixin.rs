//! 微信公众号文章（`mp.weixin.qq.com/s/…`、`/s?__biz=…`）。
//!
//! - 正文 `#js_content` 默认 `visibility: hidden; opacity: 0`，靠脚本显示出来；通用清理会当隐藏元素删掉，`fix` 先放出来
//!   （两种模式都要）。里面另外藏起来的照旧去掉。
//! - 标题、公众号名、作者在页头；发布时间只在脚本变量 `ct`（秒）里，按北京时间写成 `2026-02-05T20:26:49+08:00`。
//! - 图是懒加载的（`data-src`），网址在 `images` 里（Phase 4）；正文照旧只留说明。

use super::{BLANK, Site, SiteMeta, text_of};
use dom_query::Document;
use regex::Regex;
use std::sync::OnceLock;

pub(super) const SITE: Site = Site {
    key: "weixin",
    matches: |u| {
        u.host_str() == Some("mp.weixin.qq.com")
            && (u.path() == "/s" || u.path().starts_with("/s/"))
    },
    fix: Some(|doc| {
        doc.select("#js_content").remove_attr("style");
    }),
    root: &["#js_content"],
    meta: Some(meta),
    ..BLANK
};

fn meta(doc: &Document, _: &url::Url) -> SiteMeta {
    let first = |sel: &str| {
        doc.select(sel)
            .nodes()
            .first()
            .map(text_of)
            .filter(|t| !t.is_empty())
    };
    let author = doc
        .select(r#"meta[name="author"]"#)
        .attr("content")
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .or_else(|| first("#js_author_name_text"));
    SiteMeta {
        title: first("#activity-name"),
        byline: author,
        published: script_var(doc, "ct")
            .and_then(|s| s.parse().ok())
            .map(beijing),
        site: first("#js_name"),
    }
}

/// 脚本里 `var 名字 = "值"` 的值。
fn script_var(doc: &Document, name: &str) -> Option<String> {
    static VAR: OnceLock<Regex> = OnceLock::new();
    let var = VAR.get_or_init(|| Regex::new(r#"\bvar\s+(\w+)\s*=\s*["']([^"']*)["']"#).unwrap());
    doc.select("script").nodes().iter().find_map(|s| {
        let text = s.text();
        var.captures_iter(&text)
            .find(|c| &c[1] == name)
            .map(|c| c[2].to_string())
    })
}

/// Unix 秒 → 北京时间的 RFC 3339。
fn beijing(secs: i64) -> String {
    let t = secs + 8 * 3600;
    let (days, rest) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    let (y, m, d) = civil(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}+08:00",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// 1970-01-01 起的天数 → 年月日（Howard Hinnant 的 `civil_from_days`）。
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_time_is_beijing_time() {
        assert_eq!(beijing(1_770_294_409), "2026-02-05T20:26:49+08:00");
        assert_eq!(beijing(0), "1970-01-01T08:00:00+08:00");
        assert_eq!(beijing(951_782_400), "2000-02-29T08:00:00+08:00");
    }
}
