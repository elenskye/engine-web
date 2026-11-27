# 第三方依赖

引入依赖时在这里登记一行（`CLAUDE.md`「五」）。

| 名称 | 版本 | 许可证 | 用途 | 链接 |
| --- | --- | --- | --- | --- |
| `dom_smoothie` | `^0.18.2` | MIT | 抽正文（Readability 的移植）、元数据；和 Peck 同一版 | [crate](https://crates.io/crates/dom_smoothie) |
| `dom_query` | `^0.28.0`（`markdown`） | MIT | HTML 解析、转 Markdown、`full` 模式走树；和 Peck 同一版（与 Tauri 共用 html5ever 0.39） | [crate](https://crates.io/crates/dom_query) |
| `encoding_rs` | `^0.8.44` | (Apache-2.0 OR MIT) AND BSD-3-Clause | 按 BOM、声明的编码解码 | [crate](https://crates.io/crates/encoding_rs) |
| `chardetng` | `^1.0.0` | Apache-2.0 OR MIT | 没声明编码又不是 UTF-8 时探测编码（Firefox 用的那个），见 `docs/decisions/0005` | [crate](https://crates.io/crates/chardetng) |
| `regex` | `^1.13.2` | MIT OR Apache-2.0 | 整理 Markdown（链接补全、图片只留说明），Peck 的 `tidy_markdown` 用的就是它 | [crate](https://crates.io/crates/regex) |
| `url` | `^2.5.9` | MIT OR Apache-2.0 | 网址解析、相对链接补全 | [crate](https://crates.io/crates/url) |
| `engine-core` | tag `v0.2.0` | UNLICENSED（站主自己的） | 工具清单、调用、命令行骨架、find、取消 | [GitHub](https://github.com/elenskye/engine-core) |
| `serde` | `^1.0.229`（`derive`） | MIT OR Apache-2.0 | `Loc`、`BlockKind` 序列化（find 的结果） | [crate](https://crates.io/crates/serde) |
| `serde_json` | `^1.0.154` | MIT OR Apache-2.0 | 工具参数和结果；测试读合成样本的 `.expect.json` | [crate](https://crates.io/crates/serde_json) |
| `ureq` | `^3.5.2`（Mac：`native-tls-no-default`、`gzip`、`brotli`；别的平台：`rustls`、`gzip`、`brotli`） | MIT OR Apache-2.0 | 同步 HTTP（`transport::Ureq`）：系统 TLS、解压、自定义解析器钉地址；Pith、engine-pdf 也用 | [crate](https://crates.io/crates/ureq) |
| `log` | `^0.4.35`（`max_level_debug`、`release_max_level_debug`） | MIT OR Apache-2.0 | 不直接用：编译时去掉 TRACE，免得 ureq-proto 记下 Tavily 的鉴权头（engine-pdf 的做法） | [crate](https://crates.io/crates/log) |

清零密钥和 Cookie 用 engine-core 再导出的 `Zeroizing`（`zeroize`），不另引。

dom_smoothie、dom_query 有更新的版本（0.20 / 0.30，html5ever 0.40）。现在跟 Peck 钉同一版，见 `docs/decisions/0002`。
