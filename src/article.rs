//! 取正文的结果类型。位置约定见 `CLAUDE.md`「六」：标题路径 + 第几段。

/// 取正文的方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// 抽正文（Readability），转 Markdown。默认。
    #[default]
    Article,
    /// 全部文字（不抽正文），表格一行一行、格子用制表符隔开；核对摘录用（Pith）。
    Full,
}

impl Mode {
    pub fn key(self) -> &'static str {
        match self {
            Self::Article => "article",
            Self::Full => "full",
        }
    }
}

/// `extract` 的选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractOptions {
    pub mode: Mode,
    /// `article` 模式抽出来的正文比这短（按字数），就退回整个 `<body>`。默认 200。
    pub min_article_chars: usize,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            mode: Mode::Article,
            min_article_chars: 200,
        }
    }
}

/// 读得了的类型。PDF、图片等不是网页的在 [`crate::Error::NotWeb`] 里。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Html,
    Markdown,
    Text,
    Json,
    Xml,
}

impl Kind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Markdown => "markdown",
            Self::Text => "text",
            Self::Json => "json",
            Self::Xml => "xml",
        }
    }
}

/// 取好的一页。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Article {
    pub kind: Kind,
    /// 解码用的编码（encoding_rs 的名字，如 `UTF-8`、`GBK`、`Big5`）。
    pub encoding: &'static str,
    pub title: Option<String>,
    pub byline: Option<String>,
    /// 发布日期，网页里写的原样（常见是 ISO 8601）。
    pub published: Option<String>,
    pub site: Option<String>,
    pub lang: Option<String>,
    /// `article` 模式是 Markdown（链接是绝对网址，图片只留说明）；`full` 模式是全部文字；
    /// Markdown、纯文本、JSON、XML 是原文。
    pub markdown: String,
    /// 分段：给 find、分段交给模型、App 切块用。
    pub blocks: Vec<Block>,
    /// 正文里的 http(s) 链接（绝对网址，按出现顺序，同一网址只留第一次）。
    pub links: Vec<Link>,
    /// 正文很短而脚本很多：内容可能要开 JavaScript 才出来。
    pub needs_js: bool,
}

/// 一段。段号全文连续、从 1 数起，小标题也算一段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// 这一段上面的各级小标题（不含文章标题；小标题这一段的路径不含它自己）。
    pub heading_path: Vec<String>,
    /// 第几段，从 1 起。
    pub paragraph: u32,
    pub kind: BlockKind,
    /// 这一段的文字（去掉 Markdown 记号，表格一行的格子用制表符隔开）。
    pub text: String,
}

impl Block {
    /// 给模型的位置写法：「〈引言 › 方法〉第 3 段」；没有小标题时「第 3 段」。
    pub fn label(&self) -> String {
        if self.heading_path.is_empty() {
            format!("第 {} 段", self.paragraph)
        } else {
            format!(
                "〈{}〉第 {} 段",
                self.heading_path.join(" › "),
                self.paragraph
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockKind {
    /// 小标题（文章标题也是这一种，路径为空）。
    Heading,
    Paragraph,
    /// 列表一项。
    ListItem,
    /// 表格一行。
    TableRow,
    /// 一个代码块。
    Code,
}

impl BlockKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Heading => "heading",
            Self::Paragraph => "paragraph",
            Self::ListItem => "list_item",
            Self::TableRow => "table_row",
            Self::Code => "code",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub text: String,
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_label_reads_like_the_convention() {
        let mut block = Block {
            heading_path: vec!["引言".into(), "方法".into()],
            paragraph: 3,
            kind: BlockKind::Paragraph,
            text: "……".into(),
        };
        assert_eq!(block.label(), "〈引言 › 方法〉第 3 段");
        block.heading_path.clear();
        assert_eq!(block.label(), "第 3 段");
    }
}
