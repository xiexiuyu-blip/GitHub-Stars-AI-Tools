#!/usr/bin/env bash
# AI 开发者唯一的构建/测试入口：固定命令、固定参数，不联网安装任何东西。
# 用法：tools/cards/run.sh <web-build|web-test [过滤词]|rust-fmt|rust-fmt-fix|rust-check|rust-test [过滤词]>
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
cd "$ROOT"

export COREPACK_HOME="$ROOT/.corepack"
export COREPACK_ENABLE_NETWORK=0
# 不让 pnpm 在运行脚本前自作主张地重装依赖（无人值守时没有终端可确认，沙盒里也写不了全局仓库）
export CI=true
export npm_config_verify_deps_before_run=false
export pnpm_config_verify_deps_before_run=false
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
    # 跳过 1 个必须访问系统钥匙串的用例：沙盒里没有默认钥匙串，它必然失败
    # （它只读一个不存在的测试条目 fox-stars-lab-test，不碰真实凭据）。总控在沙盒外跑完整套件。
    SKIP="auth::tests::macos_secure_store_initializes_keychain_before_entry_creation"
    if [ "$#" -gt 0 ]; then
      exec cargo test --manifest-path "$MANIFEST" --lib "$1" -- --test-threads=8 --skip "$SKIP"
    fi
    exec cargo test --manifest-path "$MANIFEST" --lib -- --test-threads=8 --skip "$SKIP"
    ;;
  *)
    echo "用法：tools/cards/run.sh <web-build|web-test [过滤词]|rust-fmt|rust-fmt-fix|rust-check|rust-test [过滤词]>" >&2
    exit 2
    ;;
esac
