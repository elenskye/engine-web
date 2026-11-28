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
    /// 用上的站点规则（`arxiv`、`plos`……，见 `docs/decisions/0008`）；没有就是通用做法。
    pub rule: Option<&'static str>,
    /// 文献信息：网页里有学术元数据（`citation_*`、PRISM、Dublin Core）或站点规则认得出时才有。
    pub paper: Option<Paper>,
    /// 网站挡程序时回的验证页：哪一家（`Cloudflare`、`微信`……，认不出是哪家的是 `unknown`）；正常网页是 `None`。
    /// 工具见到就报 `bot_check`（`docs/decisions/0008`）。
    pub bot_check: Option<&'static str>,
    /// 正文里的图（网址、说明、在哪一段）。
    pub images: Vec<Image>,
    /// 这一页里找到的「下一页」（同一篇文章的下一页，不是下一篇）。
    pub next_page: Option<String>,
    /// 合并了几页时每页的网址（第 1 页在前）；没合并时是空的。
    pub pages: Vec<String>,
}

/// 文献信息。字段照网页里写的原样（去掉多余空白），没写的就空着；不猜、不补。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Paper {
    pub title: Option<String>,
    pub authors: Vec<Author>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub keywords: Vec<String>,
    /// 刊名、会议名，或工作论文的系列（NBER、BIS……）。
    pub journal: Option<String>,
    pub publisher: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    /// 页码：「583–589」，只有起始页时是起始页（PLOS 的 `e0363891`）。
    pub pages: Option<String>,
    /// 报告号（工作论文的编号，如 `w36037`）。
    pub number: Option<String>,
    /// 发表日期，网页里写的原样。
    pub published: Option<String>,
    /// DOI，去掉了 `doi:`、`https://doi.org/` 前缀。
    pub doi: Option<String>,
    pub pdf_url: Option<String>,
    /// 别的编号：`arxiv`、`pmid`、`pmcid`、`issn`、`isbn`。
    pub ids: Vec<PaperId>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Author {
    pub name: String,
    /// 机构，照网页的顺序。
    pub affiliations: Vec<String>,
    pub orcid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PaperId {
    pub scheme: &'static str,
    pub value: String,
}

/// 正文里的一张图。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Image {
    /// 绝对网址（懒加载的取真正的那张，`srcset` 取最大的）。
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    /// 图注（`figcaption` 这类）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// 在第几段（图前面最近的一段）；在正文最前面时是 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraph: Option<u32>,
    /// 合并了几页时在第几页（从 1 起）；只有一页时是 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

/// 一段。段号全文连续、从 1 数起，小标题也算一段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// 这一段上面的各级小标题（不含文章标题；小标题这一段的路径不含它自己）。
    pub heading_path: Vec<String>,
    /// 第几段，从 1 起（合并了几页时每页各自从 1 数）。
    pub paragraph: u32,
    pub kind: BlockKind,
    /// 这一段的文字（去掉 Markdown 记号，表格一行的格子用制表符隔开）。
    pub text: String,
    /// 合并了几页时在第几页（从 1 起）；只有一页时是 `None`。
    pub page: Option<u32>,
}

impl Block {
    /// 给模型的位置写法：「〈引言 › 方法〉第 3 段」；没有小标题时「第 3 段」；合并了几页时前面加「第 2 页」。
    pub fn label(&self) -> String {
        label(self.page, &self.heading_path, self.paragraph)
    }
}

pub(crate) fn label(page: Option<u32>, heading_path: &[String], paragraph: u32) -> String {
    let page = page.map(|p| format!("第 {p} 页")).unwrap_or_default();
    if heading_path.is_empty() {
        format!("{page}第 {paragraph} 段")
    } else {
        format!("{page}〈{}〉第 {paragraph} 段", heading_path.join(" › "))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
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
            page: None,
        };
        assert_eq!(block.label(), "〈引言 › 方法〉第 3 段");
        block.heading_path.clear();
        assert_eq!(block.label(), "第 3 段");
        block.page = Some(2);
        assert_eq!(block.label(), "第 2 页第 3 段");
        block.heading_path = vec!["方法".into()];
        assert_eq!(block.label(), "第 2 页〈方法〉第 3 段");
    }
}
