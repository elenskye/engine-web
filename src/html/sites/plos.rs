//! PLOS 的文章页（`journals.plos.org/<刊>/article?id=…`）：正文从摘要起（`#artText`）。
//! 题名、作者列表、指标、图的轮播在正文外，不要；图下面的「Download: PPT / PNG / TIFF」、空的锚点去掉。

use super::{BLANK, Site, text_of};
use dom_query::{NodeRef, Selection};

pub(super) const SITE: Site = Site {
    key: "plos",
    matches: |u| u.host_str() == Some("journals.plos.org") && u.path().ends_with("/article"),
    fix: None,
    root: &["#artText"],
    drop: &[".figure-inline-download", "a.link-target"],
    shape: Some(shape),
    ..BLANK
};

/// 参考文献的序号（`<span class="order">60. </span>`）换成带空格的文字，不然转出来是「60.Mushi HM」。
fn shape(root: &NodeRef) {
    for order in Selection::from(*root).select("span.order").nodes() {
        order.replace_with_html(format!("{} ", super::escape(&text_of(order))));
    }
}
