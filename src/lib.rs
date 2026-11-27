//! engine-web：Peck、Pith、Pomelo 共用的网页引擎（搜索、抓取、取正文）。v0.1.0 只有计划，见 `docs/design.md`。

/// 引擎版本，和 `Cargo.toml` 一致。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_manifest() {
        assert_eq!(super::VERSION, "0.1.0");
    }
}
