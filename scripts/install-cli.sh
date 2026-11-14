#!/bin/sh
# Installs the `autome` command line.
#
#   sh scripts/install-cli.sh                 in a checkout: build and install
#   sh scripts/install-cli.sh --from X        X is a tarball from package-cli.sh,
#                                             or its URL; the .sha256 beside it
#                                             is checked when it exists
#   curl -fsSL <raw url of this file> | sh    elsewhere: download a release
#
# Where:      $PREFIX, default ~/.local/bin. No sudo, no system directories.
# Which:      $AUTOME_VERSION picks the release to download (default: latest).
#             Ignored when building from a checkout or installing --from.
#
# After installing it runs `autome doctor`, which says whether Git and at
# least one of Claude Code / Codex are there and logged in. A failing doctor
# does not undo the install — the binary is fine, the machine needs work.
#
# Written for /bin/sh (bash 3.2 on macOS): no arrays, no `[[`, no `local`, and
# `${var}` in braces wherever Chinese text follows — bash 3.2 reads the bytes
# of `$var，` as part of the name.
set -eu

REPO_SLUG=LinkedDestiny/autome
prefix=${PREFIX:-$HOME/.local/bin}
from=''

while [ $# -gt 0 ]; do
  case "$1" in
    --from)
      [ $# -ge 2 ] || { echo "install-cli: --from 需要一个 tar 包路径或 URL" >&2; exit 2; }
      from=$2
      shift 2
      ;;
    --from=*)
      from=${1#--from=}
      shift
      ;;
    -h | --help)
      sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "install-cli: 不认识的参数 $1" >&2
      exit 2
      ;;
  esac
done

[ "$(uname -s)" = Darwin ] || { echo "install-cli: 目前只支持 macOS" >&2; exit 1; }
case "$(uname -m)" in
  arm64 | aarch64) target=aarch64-apple-darwin ;;
  x86_64) target=x86_64-apple-darwin ;;
  *) echo "install-cli: 不支持的架构 $(uname -m)" >&2; exit 1 ;;
esac

work=$(mktemp -d "${TMPDIR:-/tmp}/autome-install.XXXXXX")
# The exit status is kept: a trap that ends in a successful `rm` would
# otherwise turn every failure below into exit 0.
trap 'rc=$?; rm -rf "$work"; exit $rc' EXIT
trap 'exit 130' INT TERM

# Checks `$1` against `$2` (a `shasum -a 256` line). Missing checksum is a
# warning for a local file — package-cli.sh always writes one, but a tarball
# copied on its own is still the user's to trust — and an error for a download.
verify() {
  file=$1
  sums=$2
  want=$(awk '{print $1}' "$sums")
  got=$(shasum -a 256 "$file" | awk '{print $1}')
  if [ "$want" != "$got" ]; then
    echo "install-cli: sha256 不符：期望 ${want}，实际 ${got}" >&2
    exit 1
  fi
  echo "==> sha256 校验通过"
}

# Downloads `$1` to `$2`.
fetch() {
  command -v curl > /dev/null || { echo "install-cli: 需要 curl" >&2; exit 1; }
  curl -fsSL "$1" -o "$2" || {
    echo "install-cli: 下载失败：$1（版本不存在，或还没有发布？）" >&2
    return 1
  }
}

# Unpacks the tarball `$1` and leaves the binary's path in $bin.
unpack() {
  tar -C "$work" -xzf "$1"
  set -- "$work"/*/autome
  [ "$#" -eq 1 ] && [ -f "$1" ] && [ -x "$1" ] || { echo "install-cli: tar 包里没有唯一的 autome" >&2; exit 1; }
  bin=$1
  panel=$(dirname "$bin")/web
}

script_dir=$(cd "$(dirname "$0")" 2> /dev/null && pwd || echo '')
checkout=''
if [ -n "$script_dir" ] && [ -f "$script_dir/../crates/autome-cli/Cargo.toml" ]; then
  checkout=$(cd "$script_dir/.." && pwd)
fi

bin=''
panel=''
node_bin=${AUTOME_NODE:-$(command -v node || true)}
node_version=$([ -n "$node_bin" ] && "$node_bin" -p process.versions.node 2>/dev/null || true)
node_ok=0
major=${node_version%%.*}
rest=${node_version#*.}
minor=${rest%%.*}
case "$major:$minor" in
  *[!0-9:]* | :* | *:) ;;
  *) if [ "$major" -gt 22 ] || { [ "$major" -eq 22 ] && [ "$minor" -ge 22 ]; }; then node_ok=1; fi ;;
esac
if [ -n "$from" ]; then
  case "$from" in
    http://* | https://*)
      echo "==> 下载 $from"
      fetch "$from" "$work/autome.tar.gz"
      if fetch "$from.sha256" "$work/autome.tar.gz.sha256" 2> /dev/null; then
        verify "$work/autome.tar.gz" "$work/autome.tar.gz.sha256"
      else
        echo "install-cli: 没有找到 $from.sha256，拒绝安装未校验的下载" >&2
        exit 1
      fi
      unpack "$work/autome.tar.gz"
      ;;
    *)
      [ -f "$from" ] || { echo "install-cli: 没有这个文件：$from" >&2; exit 1; }
      if [ -f "$from.sha256" ]; then
        verify "$from" "$from.sha256"
      else
        echo "install-cli: 提示：$from 旁边没有 .sha256，未校验" >&2
      fi
      unpack "$from"
      ;;
  esac
elif [ -n "$checkout" ]; then
  command -v cargo > /dev/null || {
    echo "install-cli: 在仓库里安装需要 Rust（cargo）；或者用 --from 安装发布包" >&2
    exit 1
  }
  echo "==> cargo build --release -p autome-cli"
  cargo build --release --manifest-path "$checkout/Cargo.toml" -p autome-cli
  bin="$checkout/target/release/autome"
  if [ "$node_ok" -eq 1 ]; then
    (cd "$checkout/apps/web" && npm ci && npm run build)
    "$node_bin" "$checkout/apps/web/scripts/stage-panel.ts" "$work/web"
    panel="$work/web"
  fi
else
  version=${AUTOME_VERSION:-latest}
  if [ "$version" = latest ]; then
    echo "==> 查询最新发布"
    fetch "https://api.github.com/repos/$REPO_SLUG/releases/latest" "$work/latest.json"
    tag=$(sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' "$work/latest.json" | head -1)
    [ -n "$tag" ] || { echo "install-cli: 读不到最新发布的版本号" >&2; exit 1; }
  else
    tag=v${version#v}
  fi
  asset="autome-${tag#v}-$target.tar.gz"
  url="https://github.com/$REPO_SLUG/releases/download/$tag/$asset"
  echo "==> 下载 $url"
  fetch "$url" "$work/$asset"
  fetch "$url.sha256" "$work/$asset.sha256"
  verify "$work/$asset" "$work/$asset.sha256"
  unpack "$work/$asset"
fi

mkdir -p "$prefix"
dest="$prefix/autome"
# A fresh file, moved into place: overwriting the bytes of a binary macOS has
# already checked the signature of gets the next run of it killed.
cp "$bin" "$dest.new.$$"
chmod 755 "$dest.new.$$"
mv -f "$dest.new.$$" "$dest"
echo "==> 已安装 ${dest}（$("$dest" --version)）"

panel_dest=$(dirname "$prefix")/lib/autome/web
if [ "$node_ok" -eq 1 ] && [ -d "$panel" ]; then
  mkdir -p "$(dirname "$panel_dest")"
  cp -R "$panel" "$panel_dest.new.$$"
  if [ -e "$panel_dest" ]; then mv "$panel_dest" "$panel_dest.old.$$"; fi
  if mv "$panel_dest.new.$$" "$panel_dest"; then
    rm -rf "$panel_dest.old.$$"
  else
    [ ! -e "$panel_dest.old.$$" ] || mv "$panel_dest.old.$$" "$panel_dest"
    exit 1
  fi
  echo "==> 已安装面板 $panel_dest；运行 autome web"
elif [ "$node_ok" -eq 0 ]; then
  echo "没有找到 Node.js 22.22 或更高版本（当前：${node_version:-没有}），只装了命令行。"
  echo "装好 Node（brew install node）后重新运行本脚本即可装上面板。"
fi

case ":$PATH:" in
  *":$prefix:"*) ;;
  *)
    echo
    echo "提示：$prefix 不在 PATH 里。把这一行加到 ~/.zshrc："
    echo "    export PATH=\"$prefix:\$PATH\""
    ;;
esac

echo
"$dest" doctor || echo "（doctor 报告了问题；autome 已装好，按上面的提示处理环境即可。）"
