# 合成样本

`tests/samples.rs` 读这里的每份 `<名字>.expect.json`，找同名的样本（`.html`、`.md`、`.json`、`.xml`、`.txt`，或 `sample` 指定的文件），`article`、`full` 两种模式各跑一遍。

- 手写的：UTF-8 的网页、Markdown、JSON、RSS、纯文本。
- 生成的：GBK / Big5 网页和「不是网页」的样本（PDF、图片、Office、压缩包、音视频），`cargo run --example make_samples` 重写，连同它们的 `.expect.json`。
- 网址都用 `example.com` 这类保留域名；文字是自己写的，不抄真实网页。

## `.expect.json`

| 字段 | 意思 |
| --- | --- |
| `about` | 这份样本测什么（必填） |
| `sample` | 样本文件名（不同名时才写） |
| `url`、`content_type` | 交给 `extract` 的网址和 HTTP 头 |
| `not_web` | 不是网页时期望的 `Binary::key()`；有它就不看别的 |
| `kind`、`encoding`、`title`、`byline`、`published`、`site`、`lang`、`needs_js` | 写了就要相等（两种模式都查） |
| `article`、`full` | 各模式的期望，见下 |

各模式里：

| 字段 | 意思 |
| --- | --- |
| `contains` | `markdown` 里要有的句子（原样） |
| `excludes` | `markdown` 和每段文字里都不能有的（导航、页脚、隐藏的指令） |
| `exact` | `markdown` 逐字等于它 |
| `max_chars` | `markdown` 最多几个字 |
| `links` | `links` 里要有的网址 |
| `blocks` | `[{contains, kind, path}]`：包含这句的那一段，种类和标题路径要对 |

另外每次都查：段号从 1 连续、没有空段、正文为空时分段也为空。
