#!/bin/sh
# 真实网页快照（v0.2 Phase 3，只在本机）：读 local/pages/urls.txt（一行一个网址，# 开头的跳过），
# 每个网址存成 local/pages/NNN-主机.<扩展名>，旁边写 .meta.json（最后的网址、Content-Type、抓取日期）。
# 快照只放 local/，不进 Git。已经存在的编号跳过；要重抓先删掉那个文件。
set -eu

cd "$(dirname "$0")/.."
list="${1:-local/pages/urls.txt}"
dir="$(dirname "$list")"
[ -f "$list" ] || { echo "没有 $list：一行写一个网址。" >&2; exit 1; }
today="$(date +%Y-%m-%d)"
n=0
grep -v '^[[:space:]]*\(#\|$\)' "$list" | while IFS= read -r url; do
  n=$((n + 1))
  num="$(printf '%03d' "$n")"
  host="$(printf '%s' "$url" | sed -E 's#^[a-zA-Z]+://([^/:?#]+).*#\1#; s#[^A-Za-z0-9.-]#_#g')"
  if ls "$dir/$num-"* >/dev/null 2>&1; then
    echo "$num 已有，跳过：$url"
    continue
  fi
  tmp="$dir/.$num.part"
  info="$(curl -sS -L --compressed --max-redirs 5 --max-time 40 --max-filesize 26214400 \
    -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) engine-web-snapshot/0.2" \
    -o "$tmp" -w '%{http_code}\t%{content_type}\t%{url_effective}' "$url")" || { echo "$num 抓不到：$url" >&2; rm -f "$tmp"; continue; }
  code="$(printf '%s' "$info" | cut -f1)"
  ctype="$(printf '%s' "$info" | cut -f2)"
  final="$(printf '%s' "$info" | cut -f3)"
  case "$code" in
    2*) ;;
    *) echo "$num HTTP $code，不存：$url" >&2; rm -f "$tmp"; continue ;;
  esac
  case "$ctype" in
    *markdown*) ext=md ;; *json*) ext=json ;; *xml*) ext=xml ;; text/plain*) ext=txt ;; *pdf*) ext=pdf ;; *) ext=html ;;
  esac
  file="$dir/$num-$host.$ext"
  mv "$tmp" "$file"
  esc() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'; }
  printf '{\n  "url": "%s",\n  "asked": "%s",\n  "content_type": "%s",\n  "status": %s,\n  "fetched": "%s"\n}\n' \
    "$(esc "$final")" "$(esc "$url")" "$(esc "$ctype")" "$code" "$today" > "$file.meta.json"
  echo "$num $code $ctype $file"
done
