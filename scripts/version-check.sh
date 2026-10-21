#!/usr/bin/env bash
# 检查三处版本号一致：Cargo workspace、根 package.json、apps/browser/package.json。
# 不一致时打印各处的值并以 1 退出。
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# [workspace.package] 段里的 version
cargo_version="$(awk '
  /^\[workspace\.package\]/ { in_pkg = 1; next }
  /^\[/ { in_pkg = 0 }
  in_pkg && /^version[[:space:]]*=/ { gsub(/.*=[[:space:]]*"|".*/, ""); print; exit }
' "$root/Cargo.toml")"

# package.json 顶层的 "version" 字段
json_version() {
  sed -n 's/^  "version":[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | head -n 1
}
root_version="$(json_version "$root/package.json")"
browser_version="$(json_version "$root/apps/browser/package.json")"

if [[ -z "$cargo_version" || "$cargo_version" != "$root_version" || "$cargo_version" != "$browser_version" ]]; then
  echo "版本号不一致：" >&2
  echo "  Cargo.toml [workspace.package]  ${cargo_version:-<缺失>}" >&2
  echo "  package.json                    ${root_version:-<缺失>}" >&2
  echo "  apps/browser/package.json       ${browser_version:-<缺失>}" >&2
  exit 1
fi
echo "版本号一致：$cargo_version"
