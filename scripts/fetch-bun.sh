#!/usr/bin/env bash
# 按 .bun-version 下载 Apple Silicon 的 Bun，用同一个 release 的 SHASUMS256.txt 校验后放到目标路径。
# 用法：fetch-bun.sh <目标路径>。目标已存在且版本一致时跳过。
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "用法：fetch-bun.sh <目标路径>" >&2
  exit 2
fi
dest="$1"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(tr -d '[:space:]' < "${root}/.bun-version")"

if [[ -x "$dest" && "$("$dest" --version 2> /dev/null || true)" == "$version" ]]; then
  echo "fetch-bun：${dest} 已是 ${version}"
  exit 0
fi

base="https://github.com/oven-sh/bun/releases/download/bun-v${version}"
asset="bun-darwin-aarch64.zip"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "fetch-bun：下载 ${base}/${asset}"
curl -fsSL -o "${tmp}/${asset}" "${base}/${asset}"
curl -fsSL -o "${tmp}/SHASUMS256.txt" "${base}/SHASUMS256.txt"
want="$(awk -v a="$asset" '$2 == a {print $1}' "${tmp}/SHASUMS256.txt")"
got="$(shasum -a 256 "${tmp}/${asset}" | awk '{print $1}')"
if [[ -z "$want" || "$want" != "$got" ]]; then
  echo "fetch-bun：${asset} 的 SHA256 校验失败（应为 ${want:-<缺失>}，实际 ${got}）" >&2
  exit 1
fi
unzip -q "${tmp}/${asset}" -d "$tmp"
mkdir -p "$(dirname "$dest")"
cp "${tmp}/bun-darwin-aarch64/bun" "$dest"
chmod +x "$dest"
echo "fetch-bun：${dest} 为 $("$dest" --version)"
