#!/usr/bin/env bash
# 安装或更新 alter（Apple Silicon）：下载 Release 里的 DMG，把 alter.app 装进 /Applications（不可写时 ~/Applications），
# 再把 CLI 链接到 ~/.local/bin/alter。
#
# 用法：
#   curl -fsSL https://github.com/ethanyulabs/alter/releases/latest/download/install.sh | bash
#   bash install.sh [--version v<x.y.z>] [--prefix <dir>] [--bin-dir <dir>] [--with-skill] [--dry-run]
#
# 环境变量（测试用）：ALTER_RELEASE_API（默认 https://api.github.com/repos/ethanyulabs/alter）、
# ALTER_RELEASE_BASE（默认 https://github.com/ethanyulabs/alter/releases/download）、
# ALTER_INSTALL_UNAME_M / ALTER_INSTALL_UNAME_S（代替 uname -m / -s）。
set -euo pipefail

api="${ALTER_RELEASE_API:-https://api.github.com/repos/ethanyulabs/alter}"
base="${ALTER_RELEASE_BASE:-https://github.com/ethanyulabs/alter/releases/download}"
tag=""
prefix=""
bin_dir="${HOME}/.local/bin"
with_skill=0
dry_run=0

usage() {
  sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      tag="${2:-}"
      shift 2 || { echo "--version 需要一个值" >&2; exit 2; }
      ;;
    --prefix)
      prefix="${2:-}"
      shift 2 || { echo "--prefix 需要一个值" >&2; exit 2; }
      ;;
    --bin-dir)
      bin_dir="${2:-}"
      shift 2 || { echo "--bin-dir 需要一个值" >&2; exit 2; }
      ;;
    --with-skill)
      with_skill=1
      shift
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "未知参数：${1}" >&2
      exit 2
      ;;
  esac
done

step() {
  echo "==> ${1}"
}

# 平台：只支持 Apple Silicon 的 macOS
os="${ALTER_INSTALL_UNAME_S:-$(uname -s)}"
arch="${ALTER_INSTALL_UNAME_M:-$(uname -m)}"
if [[ "$os" != "Darwin" ]]; then
  echo "alter 只支持 macOS（当前为 ${os}）" >&2
  exit 1
fi
if [[ "$arch" != "arm64" ]]; then
  echo "alter 目前只支持 Apple Silicon（arm64），当前为 ${arch}" >&2
  exit 1
fi

# 版本号：v 加三段整数（会拼进下载地址与临时文件名）
valid_tag() {
  [[ "$1" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]
}
if [[ -n "$tag" ]] && ! valid_tag "$tag"; then
  echo "版本号应为 v<x.y.z>，收到「${tag}」" >&2
  exit 1
fi

# 相对路径按当前目录转成绝对路径：bin-dir 里的链接要指向真实位置
absolute() {
  case "$1" in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s/%s\n' "$(pwd)" "$1" ;;
  esac
}
bin_dir="$(absolute "$bin_dir")"

# 安装位置：/Applications 不可写时改用 ~/Applications
if [[ -z "$prefix" ]]; then
  prefix="/Applications"
  if [[ "$dry_run" -eq 0 && ! -w "$prefix" ]]; then
    prefix="${HOME}/Applications"
    echo "提示：/Applications 不可写，改装到 ${prefix}" >&2
  fi
fi
prefix="$(absolute "$prefix")"
app="${prefix}/alter.app"

# 正在运行的 alter：经 lsappinfo 取 pid 与包路径（不需要 AppleScript 的自动化授权）。
# 只有它正是这次要替换的 <prefix>/alter.app 时才需要先退出、装完再打开
app_id="dev.ethanyulabs.alter"
running_pid=""
if command -v lsappinfo > /dev/null 2>&1; then
  pid="$(lsappinfo info -only pid -app "$app_id" 2> /dev/null | sed -n 's/^"pid"=\([0-9][0-9]*\).*/\1/p' | head -n 1)"
  bundle="$(lsappinfo info -only bundlepath -app "$app_id" 2> /dev/null | sed -n 's/^"LSBundlePath"="\(.*\)"$/\1/p' | head -n 1)"
  if [[ -n "$pid" && -n "$bundle" && -d "$app" && -d "$bundle" ]] \
    && [[ "$(cd "$bundle" && pwd -P)" == "$(cd "$app" && pwd -P)" ]]; then
    running_pid="$pid"
  fi
fi

if [[ "$dry_run" -eq 1 ]]; then
  if [[ -z "$tag" ]]; then
    echo "[dry-run] 查询 latest 版本：${api}/releases/latest"
    shown="<latest>"
  else
    shown="${tag#v}"
  fi
  echo "[dry-run] 下载 ${base}/v${shown}/alter-${shown}-arm64.dmg 与 .sha256"
  echo "[dry-run] 校验 SHA256"
  echo "[dry-run] 挂载 alter-${shown}-arm64.dmg"
  if [[ -n "$running_pid" ]]; then
    echo "[dry-run] 退出正在运行的 alter（pid ${running_pid}）"
  fi
  echo "[dry-run] 安装到 ${app}"
  echo "[dry-run] 链接 ${bin_dir}/alter -> ${app}/Contents/Resources/bin/alter"
  if [[ -n "$running_pid" ]]; then
    echo "[dry-run] 重新打开 alter"
  fi
  if [[ "$with_skill" -eq 1 ]]; then
    echo "[dry-run] 运行 ${bin_dir}/alter skill install"
  fi
  exit 0
fi

# 版本：省略时取 latest 的 tag_name（不依赖 jq）
if [[ -z "$tag" ]]; then
  step "查询 latest 版本"
  # curl 失败时不能让 set -e 在赋值处直接结束：要给出下面的提示
  tag="$(curl -fsSL "${api}/releases/latest" 2> /dev/null | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1 || true)"
  if [[ -z "$tag" ]]; then
    echo "无法获取 latest 版本（${api}/releases/latest）" >&2
    exit 1
  fi
  if ! valid_tag "$tag"; then
    echo "latest 的版本号应为 v<x.y.z>，收到「${tag}」" >&2
    exit 1
  fi
fi
version="${tag#v}"
dmg="alter-${version}-arm64.dmg"

tmp="$(mktemp -d)"
mnt="${tmp}/mnt"
mounted=0
# 替换过程的状态：失败时由 cleanup 把旧的 alter.app 恢复回来、把退出过的 app 重新打开
old_saved=0
succeeded=0
was_running=0
detach() {
  if [[ "$mounted" -eq 1 ]]; then
    if hdiutil detach -quiet "$mnt" || hdiutil detach -quiet -force "$mnt"; then
      mounted=0
    else
      echo "提示：${dmg} 仍挂载在 ${mnt}，可以手动运行 hdiutil detach 卸载" >&2
      return 1
    fi
  fi
}
cleanup() {
  if [[ "$succeeded" -ne 1 ]]; then
    rm -rf "${app}.new"
    if [[ "$old_saved" -eq 1 ]]; then
      rm -rf "$app"
      mv "${app}.old" "$app" && echo "已恢复原来的 ${app}" >&2
    fi
    if [[ "$was_running" -eq 1 ]]; then
      open "$app" || true
    fi
  fi
  if detach; then
    rm -rf "$tmp"
  fi
  return 0
}
trap cleanup EXIT

step "下载 ${dmg}"
if ! curl -fsSL -o "${tmp}/${dmg}" "${base}/v${version}/${dmg}" \
  || ! curl -fsSL -o "${tmp}/${dmg}.sha256" "${base}/v${version}/${dmg}.sha256"; then
  echo "下载 ${base}/v${version}/${dmg} 失败（版本 v${version} 可能不存在）" >&2
  exit 1
fi

step "校验 SHA256"
want="$(awk '{print $1; exit}' "${tmp}/${dmg}.sha256")"
got="$(shasum -a 256 "${tmp}/${dmg}" | awk '{print $1}')"
if [[ -z "$want" || "$want" != "$got" ]]; then
  echo "SHA256 校验失败：${dmg} 应为 ${want:-<缺失>}，实际 ${got}" >&2
  exit 1
fi

step "挂载 ${dmg}"
mkdir -p "$mnt"
hdiutil attach -quiet -nobrowse -readonly -mountpoint "$mnt" "${tmp}/${dmg}"
mounted=1

if [[ -n "$running_pid" ]]; then
  step "退出正在运行的 alter（pid ${running_pid}）"
  kill -TERM "$running_pid" 2> /dev/null || true
  for _ in $(seq 1 50); do
    kill -0 "$running_pid" 2> /dev/null || break
    sleep 0.2
  done
  if kill -0 "$running_pid" 2> /dev/null; then
    echo "alter（pid ${running_pid}）10 秒内没有退出，没有做任何改动；请先退出 alter 再重试" >&2
    exit 1
  fi
  was_running=1
fi

step "安装到 ${app}"
mkdir -p "$prefix"
rm -rf "${app}.new" "${app}.old"
ditto "${mnt}/alter.app" "${app}.new"
xattr -dr com.apple.quarantine "${app}.new" 2> /dev/null || true
if [[ -e "$app" ]]; then
  mv "$app" "${app}.old"
  old_saved=1
fi
mv "${app}.new" "$app"
detach || exit 1

step "链接 ${bin_dir}/alter"
mkdir -p "$bin_dir"
ln -sfn "${app}/Contents/Resources/bin/alter" "${bin_dir}/alter"
rm -rf "${app}.old"
old_saved=0
succeeded=1
case ":${PATH}:" in
  *":${bin_dir}:"*) ;;
  *) echo "提示：${bin_dir} 不在 PATH 中，把 export PATH=\"${bin_dir}:\$PATH\" 加进 shell 配置" >&2 ;;
esac

if [[ "$was_running" -eq 1 ]]; then
  step "重新打开 alter"
  open "$app" || true
fi

if [[ "$with_skill" -eq 1 ]]; then
  step "安装 agent skill"
  if ! "${bin_dir}/alter" skill install; then
    echo "提示：alter skill install 没有成功，可以之后用 alter skill install --target <dir> 重试" >&2
  fi
fi

echo "alter ${version} installed to ${app}"
