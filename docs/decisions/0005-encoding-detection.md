# 0005 编码识别：没声明时用 chardetng

2026-11-27，v0.2 Phase 2，站主在提问里定的（design §10 的未定项）。

顺序：**BOM → 合法的 UTF-8 → 声明的编码（HTTP 头 → 网页 meta / XML 声明，开头 4 KB）→ chardetng 探测**（网址的顶级域名当提示）。

- **合法的 UTF-8 排在声明前面**，和 Pith 的代码一样（design §3 原来写的是声明在前）：老站改成 UTF-8 后常留着 `charset=gb2312`，按声明解反而乱码；真正的 GBK、Big5 字节几乎不可能恰好是合法的 UTF-8。样本 `utf8-misdeclared`。
- **声明了 UTF-8 而有坏字节**（比如只收了前一部分、断在字中间）：还按 UTF-8，坏字节替换掉。Pith 这时退到 GB18030，整页会乱。没声明、只有结尾断了半个字的也按 UTF-8。
- **没声明又不是 UTF-8**：原计划退 GB18030（Pith 的做法）。拿样本试过：

  | 样本 | 只退 GB18030 | chardetng |
  | --- | --- | --- |
  | GBK（长文、短标题、一句话） | 对 | 对 |
  | Big5（.tw、.com） | 乱码 | 对 |
  | Shift_JIS（.jp、.com） | 乱码 | 对 |
  | EUC-KR | 乱码 | 对 |
  | windows-1252 法语（.fr、.com、只有一个 é） | 乱码 | 对 |

  所以加 `chardetng`（Firefox 用的探测器，encoding_rs 同一作者，Apache-2.0 OR MIT，只多一个 `cfg-if` 依赖），取代 GB18030 后备。合成样本：`gbk-undeclared`、`big5-undeclared`、`sjis-undeclared`、`latin-undeclared`。
- 只在 HTML、XML 里找网页自己声明的编码；JSON、纯文本里写着 `charset=` 的字不算声明。
- `meta` 声明 UTF-16 的按 HTML 的规矩当 UTF-8。
- `Article.encoding` 是 encoding_rs 的名字：`UTF-8`、`GBK`、`Big5`、`Shift_JIS`、`windows-1252`……（`gb2312` 的声明也报 `GBK`，encoding_rs 把它们当同一种）。
