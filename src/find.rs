//! 在取好的网页里找字（engine-core 的统一 find）：单元是 `blocks` 里的一段，位置是标题路径 + 第几段。
//! 位置约定见 `docs/decisions/0004`。

use crate::{Article, Block, BlockKind, Error};
use engine_core::find::{Finder, Found, Query, Source, normalize};
use serde::{Deserialize, Serialize};

/// 一处命中在网页里的位置。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loc {
    /// 这一段上面的各级小标题（不含文章标题）。
    pub heading_path: Vec<String>,
    /// 第几段，全文连续，从 1 起。
    pub paragraph: u32,
    pub kind: BlockKind,
}

impl Loc {
    /// 给模型的写法：「〈引言 › 方法〉第 3 段」；没有小标题时「第 3 段」。
    pub fn label(&self) -> String {
        crate::article::label(&self.heading_path, self.paragraph)
    }
}

impl From<&Block> for Loc {
    fn from(b: &Block) -> Self {
        Self {
            heading_path: b.heading_path.clone(),
            paragraph: b.paragraph,
            kind: b.kind,
        }
    }
}

/// 在 `article` 里找 `query`。`under` 只在某个小标题下面找（含这个小标题本身）：从上往下的几级标题，
/// 可以只写最后几级（`["方法"]` 能找到〈引言 › 方法〉）；比较时和 find 一样归一化（简繁、全角、大小写）。
/// 网页里没有这个小标题交 [`Error::NoHeading`]。
pub fn find(
    article: &Article,
    query: &Query,
    under: Option<&[String]>,
) -> Result<Found<Loc>, Error> {
    let scopes = match under.filter(|u| !u.is_empty()) {
        Some(under) => {
            let scopes = sections(&article.blocks, under);
            if scopes.is_empty() {
                return Err(Error::NoHeading);
            }
            Some(scopes)
        }
        None => None,
    };
    let mut finder = Finder::new(query);
    for block in &article.blocks {
        if scopes
            .as_ref()
            .is_some_and(|s| !s.iter().any(|path| inside(block, path)))
        {
            continue;
        }
        finder.scan(&block.text, Source::Text, |_| Loc::from(block));
    }
    Ok(finder.finish())
}

fn norm(s: &str) -> String {
    normalize(s).text
}

/// 末尾几级和 `under` 对得上的小标题，各自的完整路径（含它自己）。
fn sections(blocks: &[Block], under: &[String]) -> Vec<Vec<String>> {
    let want: Vec<String> = under.iter().map(|u| norm(u)).collect();
    blocks
        .iter()
        .filter(|b| b.kind == BlockKind::Heading)
        .map(|b| {
            b.heading_path
                .iter()
                .chain(std::iter::once(&b.text))
                .cloned()
                .collect::<Vec<String>>()
        })
        .filter(|full| {
            full.len() >= want.len()
                && full[full.len() - want.len()..]
                    .iter()
                    .map(|h| norm(h))
                    .eq(want.iter().cloned())
        })
        .collect()
}

/// 这一段在这个小标题下面（或者就是这个小标题）。
fn inside(block: &Block, path: &[String]) -> bool {
    let under_it =
        block.heading_path.len() >= path.len() && block.heading_path[..path.len()] == *path;
    let is_it = block.kind == BlockKind::Heading
        && block.heading_path.len() + 1 == path.len()
        && block.heading_path[..] == path[..path.len() - 1]
        && block.text == path[path.len() - 1];
    under_it || is_it
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExtractOptions, extract};
    use engine_core::find::Match;

    fn page() -> Article {
        let md = "# 标题\n\n开头一段。\n\n## 引言\n\n引言里的話。\n\n### 方法\n\n方法里的话，ＡＢＣ测试。\n\n## 结论\n\n结论里的话。";
        extract(
            md.as_bytes(),
            Some("text/markdown"),
            "",
            &ExtractOptions::default(),
        )
        .unwrap()
    }

    fn labels(found: &Found<Loc>) -> Vec<String> {
        found.hits.iter().map(|h| h.location.label()).collect()
    }

    #[test]
    fn hits_carry_heading_path_and_paragraph() {
        let found = find(&page(), &Query::new("里的话"), None).unwrap();
        assert_eq!(
            labels(&found),
            [
                "〈引言〉第 4 段",
                "〈引言 › 方法〉第 6 段",
                "〈结论〉第 8 段"
            ],
            "简繁互通：話 ↔ 话"
        );
        assert_eq!(found.hits[1].location.kind, BlockKind::Paragraph);
        let full_width = find(&page(), &Query::new("abc 测试"), None).unwrap();
        assert_eq!(
            (full_width.total, full_width.hits[0].matched.as_str()),
            (1, "ＡＢＣ测试")
        );
        let words = find(&page(), &Query::new("方法 测试"), None).unwrap();
        assert!(words.hits.iter().all(|h| h.how == Match::Words) && words.total == 2);
    }

    #[test]
    fn under_limits_the_search_to_a_section() {
        let a = page();
        let under = |u: &[&str]| u.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let found = find(&a, &Query::new("里的话"), Some(&under(&["引言"]))).unwrap();
        assert_eq!(
            labels(&found),
            ["〈引言〉第 4 段", "〈引言 › 方法〉第 6 段"]
        );
        let found = find(&a, &Query::new("里的话"), Some(&under(&["方法"]))).unwrap();
        assert_eq!(labels(&found), ["〈引言 › 方法〉第 6 段"], "只写最后一级");
        let found = find(&a, &Query::new("方法"), Some(&under(&["引言", "方法"]))).unwrap();
        assert_eq!(
            labels(&found),
            ["〈引言〉第 5 段", "〈引言 › 方法〉第 6 段"],
            "小标题本身也在范围里"
        );
        assert_eq!(
            find(&a, &Query::new("x"), Some(&under(&["没有这节"]))).unwrap_err(),
            Error::NoHeading
        );
        assert_eq!(
            find(&a, &Query::new("里的话"), Some(&[])).unwrap().total,
            3,
            "空的 under 等于不限"
        );
    }
}
