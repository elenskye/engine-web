# 0006 抓取：Transport、钉地址、上限

2026-11-27，v0.3 Phase 1，本机 Claude 定（实现细节，站主定了 v0.3 在本机做）。

- **两层**：`Transport` 只发一个请求、不跟跳转（`send`），另给主机的地址（`resolve`）；跳转、内网、同站、上限、同站间隔都在 `fetch`。所以规矩在模拟服务上就能测全（`tests/fetch.rs`），换 `Transport` 不会漏掉检查。
- **钉地址（防 DNS 重绑定）**：`fetch` 先 `resolve`，每个地址都是公网的才发，把这几个地址放进 `Request::pinned`；默认的 `Ureq` 用自定义的解析器只交回这几个地址，检查和连接用的是同一次解析。ureq 的解析器是每个 Agent 一个，所以每个请求新建一个 Agent、不留连接池（抓网页一秒最多几次，不在乎）。走代理时解析的是代理，只按名字和 IP 查内网、不钉。
- **TLS**：Mac 上系统 TLS（`native-tls-no-default` + 系统根证书，和 engine-pdf、Pith 一样）；别的平台 rustls（云端 Linux 不用装 OpenSSL）。自动解压 gzip、br，大小上限按解压后算（压缩炸弹也挡住）。
- **上限**：默认 30 s（连接 15 s）、10 MB、跳转 5 次；App 可以改，但不超过 120 s、50 MB、10 次（Pith 要 40 s、25 MB，在里面）。超了大小按收到的处理，`truncated` 为真，不算错。
- **跟到别的网站**（App 放开 `follow_other_sites` 时）：每一跳照样查内网；App 给的 `Cookie`、`Authorization` 不带过去。
- **`log` 编译时去掉 TRACE**（`max_level_debug`）：ureq-proto 的 TRACE 会记下原始报文，里面有 Tavily 的鉴权头（engine-pdf 的做法）。
