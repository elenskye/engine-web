//! 错误：种类和 `docs/design.md` §1「出错的种类」一一对应。`Display` 是一句简短中文，不带密钥、Cookie 和服务原文。

use std::fmt;

/// 引擎的错误。v0.2 的取正文只会交 [`Error::NotWeb`]；其余种类给 v0.3 的抓取和 Tavily 用，先定下来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// 不是 http(s)、带用户名密码、没有主机。
    BadUrl,
    /// 本机或内网地址（App 没放开时）。
    PrivateAddress,
    /// 跳到了别的网站，没跟过去。`to` 是新网址。
    RedirectElsewhere { to: String },
    /// 404、500 这类。
    HttpStatus { status: u16 },
    /// 不是网页：PDF、图片、Office 文件、压缩包、音视频。
    NotWeb(Binary),
    /// 抽不出正文（工具层在正文为空时交，`extract` 本身不交）。
    NoArticle,
    /// find 的 `under` 指的小标题网页里没有。
    NoHeading,
    /// 超过大小上限（v0.3 抓取用；交回前面那部分，不算失败）。
    TooLarge,
    /// 连不上。
    Network,
    /// 超时。
    Timeout,
    /// Tavily 的 401、403。
    Auth,
    /// Tavily 的 432、433（额度或套餐上限）。
    Quota,
    /// Tavily 的 429。
    RateLimited,
    /// Tavily 的 5xx。
    Service,
    /// 已停止。
    Cancelled,
}

impl Error {
    /// 给工具层的种类名（engine-core 的 `ToolError.kind`）。
    pub fn kind(&self) -> &'static str {
        match self {
            Self::BadUrl => "bad_url",
            Self::PrivateAddress => "private_address",
            Self::RedirectElsewhere { .. } => "redirect_elsewhere",
            Self::HttpStatus { .. } => "http_status",
            Self::NotWeb(_) => "not_web",
            Self::NoArticle => "no_article",
            Self::NoHeading => "no_heading",
            Self::TooLarge => "too_large",
            Self::Network => "network",
            Self::Timeout => "timeout",
            Self::Auth => "auth",
            Self::Quota => "quota",
            Self::RateLimited => "rate_limited",
            Self::Service => "service",
            Self::Cancelled => "cancelled",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadUrl => f.write_str("这不是一个能读的 http(s) 网址。"),
            Self::PrivateAddress => f.write_str("本机和内网地址不读。"),
            Self::RedirectElsewhere { to } => {
                write!(f, "这个网址跳到了别的网站（{to}），没跟过去。")
            }
            Self::HttpStatus { status } => write!(f, "网站回了 HTTP {status}。"),
            Self::NotWeb(what) => {
                let label = what.label();
                let gap = if label.starts_with(|c: char| c.is_ascii()) {
                    " "
                } else {
                    ""
                };
                write!(f, "这是{gap}{label}，不是网页。")
            }
            Self::NoArticle => f.write_str("这个网页里没读到正文。"),
            Self::NoHeading => f.write_str("网页里没有这个小标题。"),
            Self::TooLarge => f.write_str("网页太大，只收了前面一部分。"),
            Self::Network => f.write_str("连不上这个网站。"),
            Self::Timeout => f.write_str("读网页超时了。"),
            Self::Auth => f.write_str("搜索服务的密钥不对或没有权限。"),
            Self::Quota => f.write_str("搜索服务的额度用完了。"),
            Self::RateLimited => f.write_str("搜索服务限流了，等一会儿再试。"),
            Self::Service => f.write_str("搜索服务出错了。"),
            Self::Cancelled => f.write_str("已停止。"),
        }
    }
}

impl std::error::Error for Error {}

/// 不是网页的东西是什么。App 拿它决定交给哪个引擎（PDF 给 engine-pdf，Word 给 engine-word……）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Binary {
    Pdf,
    Image,
    /// .docx、.doc
    Word,
    /// .xlsx、.xls
    Excel,
    /// .pptx、.ppt
    PowerPoint,
    Epub,
    /// zip、gzip、7z、rar 这类
    Archive,
    Audio,
    Video,
    /// 认不出来的二进制
    Other,
}

impl Binary {
    /// 英文短名（工具结果的结构化数据里用）。
    pub fn key(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Image => "image",
            Self::Word => "word",
            Self::Excel => "excel",
            Self::PowerPoint => "powerpoint",
            Self::Epub => "epub",
            Self::Archive => "archive",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Other => "other",
        }
    }

    /// 给人看的名字。
    pub fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Image => "图片",
            Self::Word => "Word 文档",
            Self::Excel => "Excel 表格",
            Self::PowerPoint => "PowerPoint 演示文稿",
            Self::Epub => "EPUB 电子书",
            Self::Archive => "压缩包",
            Self::Audio => "音频",
            Self::Video => "视频",
            Self::Other => "不认识的二进制文件",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_one_short_chinese_sentence() {
        assert_eq!(
            Error::NotWeb(Binary::Pdf).to_string(),
            "这是 PDF，不是网页。"
        );
        assert_eq!(
            Error::NotWeb(Binary::Image).to_string(),
            "这是图片，不是网页。"
        );
        assert_eq!(
            Error::NotWeb(Binary::Word).to_string(),
            "这是 Word 文档，不是网页。"
        );
        assert_eq!(
            Error::RedirectElsewhere {
                to: "https://example.org/a".into()
            }
            .to_string(),
            "这个网址跳到了别的网站（https://example.org/a），没跟过去。"
        );
        assert_eq!(
            Error::HttpStatus { status: 404 }.to_string(),
            "网站回了 HTTP 404。"
        );
    }

    #[test]
    fn kinds_match_the_design() {
        let all = [
            (Error::BadUrl, "bad_url"),
            (Error::PrivateAddress, "private_address"),
            (
                Error::RedirectElsewhere { to: String::new() },
                "redirect_elsewhere",
            ),
            (Error::HttpStatus { status: 500 }, "http_status"),
            (Error::NotWeb(Binary::Other), "not_web"),
            (Error::NoArticle, "no_article"),
            (Error::NoHeading, "no_heading"),
            (Error::TooLarge, "too_large"),
            (Error::Network, "network"),
            (Error::Timeout, "timeout"),
            (Error::Auth, "auth"),
            (Error::Quota, "quota"),
            (Error::RateLimited, "rate_limited"),
            (Error::Service, "service"),
            (Error::Cancelled, "cancelled"),
        ];
        for (error, kind) in all {
            assert_eq!(error.kind(), kind);
            assert!(error.to_string().ends_with('。'), "{error}");
        }
    }
}
