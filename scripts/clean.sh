#!/bin/sh
# 清理磁盘（总仓库 GUIDE「五」「清理」）：用 engine-core 的 engine-clean。不加 --yes 只列清单。
#   ./scripts/clean.sh --start --yes     开工时：target/ 超过 3 GB 整份清（Claude Code 的开工钩子会跑）
#   ./scripts/clean.sh --round --yes     每轮 commit 之后
#   ./scripts/clean.sh --release --yes   发版打 tag 之后
#   ./scripts/clean.sh                   列出能清的全部，不动手
# engine-clean 第一次用、或本机 engine-core 的版本或源码变了时，先 cargo install 一份到 ~/.cargo/bin
# （编译产物放系统临时目录，用完就删；装的是哪份记在 ~/.cargo/engine-clean.stamp）。
set -eu
cd "$(dirname "$0")/.."
core=${ENGINE_CORE_DIR:-../engine-core}
if [ ! -f "$core/src/clean.rs" ]; then
  echo "没找到 engine-core（$core）：把 ENGINE_CORE_DIR 指到它" >&2
  exit 1
fi
PATH="$HOME/.cargo/bin:$PATH"
want="$(sed -n 's/^version = "\(.*\)"/\1/p' "$core/Cargo.toml" | head -n 1) $(cat "$core/Cargo.toml" "$core"/src/*.rs "$core"/src/bin/*.rs | shasum | cut -c1-12)"
stamp="$HOME/.cargo/engine-clean.stamp"
if ! command -v engine-clean >/dev/null 2>&1 || [ "$(cat "$stamp" 2>/dev/null || true)" != "$want" ]; then
  build="${TMPDIR:-/tmp}/engine-clean-build"
  cargo install --quiet --locked --force --path "$core" --features clean --bin engine-clean --target-dir "$build" >&2
  rm -rf "$build"
  printf '%s\n' "$want" > "$stamp"
fi
exec engine-clean "$@" .
