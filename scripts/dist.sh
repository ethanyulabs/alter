#!/usr/bin/env bash
# 打包 Apple Silicon 的 alter.app 与 DMG：编译 alterd / alter、取 Bun、打包 SDK 运行时、构建 Electron 壳，
# 再由 electron-builder 出包。产物在 dist/：alter-<版本>-arm64.dmg 与 .sha256；带 --dir 时只出 dist/mac-arm64/alter.app。
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
mode="dmg"
if [[ "${1:-}" == "--dir" ]]; then
  mode="dir"
elif [[ $# -gt 0 ]]; then
  echo "用法：dist.sh [--dir]" >&2
  exit 2
fi

bash scripts/version-check.sh
version="$(awk '
  /^\[workspace\.package\]/ { in_pkg = 1; next }
  /^\[/ { in_pkg = 0 }
  in_pkg && /^version[[:space:]]*=/ { gsub(/.*=[[:space:]]*"|".*/, ""); print; exit }
' Cargo.toml)"

echo "dist：编译 alterd 与 alter（release）"
cargo build --release -p alterd -p alter-cli
rm -rf build
mkdir -p build/bin build/sdk
cp target/release/alterd target/release/alter build/bin/

echo "dist：取 Bun"
bash scripts/fetch-bun.sh build/bin/bun

echo "dist：打包 SDK 运行时"
bun build packages/sdk/src/runtime.ts --target bun --outfile build/sdk/runtime.js

echo "dist：构建 Electron 壳"
bun run build

echo "dist：electron-builder（${mode}）"
rm -rf dist
if [[ "$mode" == "dir" ]]; then
  (cd apps/browser && ./node_modules/.bin/electron-builder -c electron-builder.yml --mac --arm64 --dir)
else
  # 出 DMG 时 hdiutil 偶尔因为卷被占用（Spotlight 等）卸载失败：重试一次
  if ! (cd apps/browser && ./node_modules/.bin/electron-builder -c electron-builder.yml --mac --arm64); then
    echo "dist：electron-builder 失败，重试一次" >&2
    (cd apps/browser && ./node_modules/.bin/electron-builder -c electron-builder.yml --mac --arm64)
  fi
  (cd dist && shasum -a 256 "alter-${version}-arm64.dmg" > "alter-${version}-arm64.dmg.sha256")
fi
echo "dist：完成（${version}）"
