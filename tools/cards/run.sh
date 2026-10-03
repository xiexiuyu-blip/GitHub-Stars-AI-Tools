#!/usr/bin/env bash
# AI 开发者唯一的构建/测试入口：固定命令、固定参数，不联网安装任何东西。
# 用法：tools/cards/run.sh <web-build|web-test [过滤词]|rust-fmt|rust-fmt-fix|rust-check|rust-test [过滤词]>
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
cd "$ROOT"

export COREPACK_HOME="$ROOT/.corepack"
export COREPACK_ENABLE_NETWORK=0
export CARGO_NET_OFFLINE=true
MANIFEST="$ROOT/apps/desktop/src-tauri/Cargo.toml"

cmd="${1:-}"
shift || true

case "$cmd" in
  web-build)
    exec pnpm build
    ;;
  web-test)
    exec pnpm --filter @gsat/desktop test "$@"
    ;;
  rust-fmt)
    exec cargo fmt --manifest-path "$MANIFEST" --check
    ;;
  rust-fmt-fix)
    exec cargo fmt --manifest-path "$MANIFEST"
    ;;
  rust-check)
    exec cargo check --manifest-path "$MANIFEST"
    ;;
  rust-test)
    if [ "$#" -gt 0 ]; then
      exec cargo test --manifest-path "$MANIFEST" --lib "$1" -- --test-threads=8
    fi
    exec cargo test --manifest-path "$MANIFEST" --lib -- --test-threads=8
    ;;
  *)
    echo "用法：tools/cards/run.sh <web-build|web-test [过滤词]|rust-fmt|rust-fmt-fix|rust-check|rust-test [过滤词]>" >&2
    exit 2
    ;;
esac
