//! Nature 的文章页（`www.nature.com/articles/…`）：正文照通用做法抽（已经不错），
//! 只去掉夹在正文里的「Similar content being viewed by others」、分享框和文末的读者评论。

use super::{BLANK, Site};

pub(super) const SITE: Site = Site {
    key: "nature",
    matches: |u| u.host_str() == Some("www.nature.com") && u.path().starts_with("/articles/"),
    fix: None,
    root: &[],
    drop: &[
        ".c-article-recommendations",
        ".c-article-share-box",
        "#article-comments-section",
    ],
    shape: None,
    ..BLANK
};
