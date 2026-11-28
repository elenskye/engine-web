//! 美国经济学会的文章页（`www.aeaweb.org/articles?id=…`）：正文是文章那一栏；
//! 期刊的页头和菜单（「About the JEP」那几列）去掉，题名在文献信息里。

use super::{BLANK, Site};

pub(super) const SITE: Site = Site {
    key: "aea",
    matches: |u| u.host_str() == Some("www.aeaweb.org") && u.path() == "/articles",
    fix: None,
    root: &["section.primary.article-detail"],
    drop: &[
        "#journal-header",
        "#tertiary-navigation",
        "#tertiary-navigation-mobile",
        "h1.title",
        r##"a[href="#article-information"]"##,
    ],
    shape: None,
    ..BLANK
};
