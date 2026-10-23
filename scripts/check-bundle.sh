#!/usr/bin/env bash
# 检查打包出的 alter.app：文件布局、包内程序的版本、Info.plist 版本、签名，以及旁边 DMG 的 SHA256。
# 用法：check-bundle.sh <alter.app 路径> <版本号>。任何一项不成立都打印原因并以 1 退出。
# release 工作流与 R1.S1 的测试共用它。
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "用法：check-bundle.sh <alter.app> <版本号>" >&2
  exit 2
fi
app="$1"
version="$2"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
res="${app}/Contents/Resources"
failed=0

fail() {
  echo "check-bundle：${1}" >&2
  failed=1
}

for bin in alterd alter bun; do
  [[ -x "${res}/bin/${bin}" ]] || fail "缺少可执行文件 Contents/Resources/bin/${bin}"
done
[[ -f "${res}/sdk/runtime.js" ]] || fail "缺少 Contents/Resources/sdk/runtime.js"
[[ -f "${res}/skills/alter-browser/SKILL.md" ]] || fail "缺少 Contents/Resources/skills/alter-browser/SKILL.md"

if [[ -x "${res}/bin/alter" ]]; then
  got="$("${res}/bin/alter" --version 2>&1 || true)"
  [[ "$got" == "alter ${version}" ]] || fail "bin/alter --version 为「${got}」，应为「alter ${version}」"
fi
if [[ -x "${res}/bin/bun" ]]; then
  want_bun="$(tr -d '[:space:]' < "${root}/.bun-version")"
  got="$("${res}/bin/bun" --version 2>&1 || true)"
  [[ "$got" == "$want_bun" ]] || fail "bin/bun --version 为「${got}」，应为「${want_bun}」"
fi

plist="${app}/Contents/Info.plist"
if [[ -f "$plist" ]]; then
  got="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$plist" 2>&1 || true)"
  [[ "$got" == "$version" ]] || fail "CFBundleShortVersionString 为「${got}」，应为「${version}」"
else
  fail "缺少 Contents/Info.plist"
fi

if ! codesign --verify --deep --strict "$app" 2> /dev/null; then
  fail "codesign --verify --deep --strict 没有通过"
fi

# DMG 构建时，dist/ 下还有 DMG 与 .sha256；--dir 构建没有，跳过这一项
dist="$(cd "$(dirname "$app")/.." && pwd)"
dmg="${dist}/alter-${version}-arm64.dmg"
if [[ -f "$dmg" ]]; then
  if [[ ! -f "${dmg}.sha256" ]]; then
    fail "缺少 $(basename "$dmg").sha256"
  else
    want="$(awk '{print $1}' "${dmg}.sha256")"
    got="$(shasum -a 256 "$dmg" | awk '{print $1}')"
    [[ "$want" == "$got" ]] || fail ".sha256 为 ${want}，实际 SHA256 为 ${got}"
  fi
  # 挂载 DMG，检查里面的 alter.app 与外面这份一样能用
  mnt="$(mktemp -d)"
  if hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "$dmg" > /dev/null; then
    inner="${mnt}/alter.app"
    got="$("${inner}/Contents/Resources/bin/alter" --version 2>&1 || true)"
    [[ "$got" == "alter ${version}" ]] || fail "DMG 里的 bin/alter --version 为「${got}」"
    got="$("${inner}/Contents/Resources/bin/bun" --version 2>&1 || true)"
    [[ "$got" == "$(tr -d '[:space:]' < "${root}/.bun-version")" ]] || fail "DMG 里的 bin/bun --version 为「${got}」"
    codesign --verify --deep --strict "$inner" 2> /dev/null || fail "DMG 里的 alter.app 没有通过 codesign --verify"
    hdiutil detach "$mnt" > /dev/null || hdiutil detach -force "$mnt" > /dev/null || fail "卸载 ${mnt} 失败"
  else
    fail "挂载 $(basename "$dmg") 失败"
  fi
  rmdir "$mnt" 2> /dev/null || true
fi

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi
echo "check-bundle：${app} 检查通过（${version}）"
