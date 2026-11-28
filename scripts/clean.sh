#!/bin/sh
# 清理磁盘（总仓库 GUIDE「五」「清理」）：用 engine-core 的 engine-clean。不加 --yes 只列清单。
#   ./scripts/clean.sh --start --yes     开工时：target/ 超过 3 GB 整份清（Claude Code 的开工钩子会跑）
#   ./scripts/clean.sh --round --yes     每轮 commit 之后
#   ./scripts/clean.sh --release --yes   发版打 tag 之后
#   ./scripts/clean.sh                   列出能清的全部，不动手
# engine-clean 第一次用、或 engine-core 换了版本时，先 cargo install 一份到 ~/.cargo/bin（编译产物用完就删）。
set -eu
cd "$(dirname "$0")/.."
core=${ENGINE_CORE_DIR:-../engine-core}
if [ ! -f "$core/src/clean.rs" ]; then
  echo "没找到 engine-core（$core）：把 ENGINE_CORE_DIR 指到它" >&2
  exit 1
fi
PATH="$HOME/.cargo/bin:$PATH"
want="engine-clean $(sed -n 's/^version = "\(.*\)"/\1/p' "$core/Cargo.toml" | head -n 1)"
if [ "$(engine-clean --version 2>/dev/null || true)" != "$want" ]; then
  build="${TMPDIR:-/tmp}/engine-clean-build"
  cargo install --quiet --locked --force --path "$core" --features clean --bin engine-clean --target-dir "$build" >&2
  rm -rf "$build"
fi
exec engine-clean "$@" .
