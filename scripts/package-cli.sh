#!/bin/sh
# Builds the `autome` command line as a release tarball.
#
#   sh scripts/package-cli.sh            → dist/autome-<version>-<target>.tar.gz
#                                          and the same name + .sha256
#
# The tarball holds one directory, `autome-<version>-<target>/`, with the
# binary and a short README. `scripts/install-cli.sh --from <tarball>` installs
# it, checking the .sha256 that sits beside it; so does the release download.
#
# `<target>` is the Rust target triple of this machine (aarch64-apple-darwin
# on Apple silicon). There is no cross build here: the binary is what this
# machine compiled, and a tarball named for another architecture would be a
# lie. CI builds on macos-14, which is Apple silicon.
set -eu

binary=''
out=''
web=''
while [ $# -gt 0 ]; do
  case "$1" in
    --binary | --out | --web)
      [ $# -ge 2 ] && [ -n "$2" ] || { echo "package-cli: $1 需要一个非空路径" >&2; exit 2; }
      case "$1" in
        --binary) binary=$2 ;;
        --out) out=$2 ;;
        --web) web=$2 ;;
      esac
      shift 2
      ;;
    *) echo "package-cli: 不认识的参数 $1" >&2; exit 2 ;;
  esac
done
# Resolve supplied paths before changing directory.
case "$binary" in '' | /*) ;; *) binary="$PWD/$binary" ;; esac
case "$out" in '' | /*) ;; *) out="$PWD/$out" ;; esac
case "$web" in '' | /*) ;; *) web="$PWD/$web" ;; esac
[ -z "$binary" ] || [ -n "$web" ] || { echo "package-cli: --binary 需要配合 --web" >&2; exit 2; }
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
out=${out:-$root/dist}

version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/autome-cli/Cargo.toml | head -1)
[ -n "$version" ] || { echo "package-cli: 读不到 crates/autome-cli/Cargo.toml 的版本号" >&2; exit 1; }
case "$(uname -m)" in
  arm64 | aarch64) arch=aarch64 ;;
  x86_64) arch=x86_64 ;;
  *) echo "package-cli: 不支持的架构 $(uname -m)" >&2; exit 1 ;;
esac
target="$arch-apple-darwin"
name="autome-$version-$target"

if [ -z "$binary" ]; then
  echo "==> cargo build --release -p autome-cli"
  cargo build --release -p autome-cli
  binary="$root/target/release/autome"
  panel_work=$(mktemp -d)
  trap 'rm -rf "$panel_work"' EXIT
  (cd apps/web && npm ci && npm run build)
  node apps/web/scripts/stage-panel.ts "$panel_work/web"
  web="$panel_work/web"
fi
[ -f "$binary" ] && [ -x "$binary" ] || { echo "package-cli: 不是可执行文件：$binary" >&2; exit 1; }
[ -f "$web/bin/autome-web.js" ] && [ -f "$web/build/server/index.js" ] || { echo "package-cli: --web 需要构建好的面板目录" >&2; exit 1; }

stage="$out/$name"
rm -rf "$stage"
mkdir -p "$stage"
# Copied, never renamed in place: the linker's ad-hoc signature belongs to the
# file it wrote, and a binary rewritten under a running process's feet is one
# macOS kills.
cp "$binary" "$stage/autome"
chmod 755 "$stage/autome"
cp -R "$web" "$stage/web"
cat > "$stage/README.md" <<EOF
# autome $version ($target)

Autome 的命令行：一句话需求，经设计、评审、实现、审计，到合并。

解包后直接运行 \`./autome web\`（保留相邻的 web/，需要 Node >= 22.22）。
安装命令行与面板：运行仓库里的 \`scripts/install-cli.sh --from <本 tar 包>\`。

然后：

    autome web                       # 在浏览器打开面板
    autome web status                # 查看地址与内核
    autome doctor                    # Git、Claude Code、Codex 与登录状态
    autome project add ~/code/shop --skip-onboarding
    cd ~/code/shop && autome task new "购物车加一个结算按钮"
    autome run                       # 驱动 Loop，到卡点会提示你下一步
    autome help

状态在 \$AUTOME_HOME（默认 ~/.autome），与桌面端共用。
\`autome core\` 是面板用的 stdio 内核，与 CLI 来自同一个文件。
\`autome --version\` 的括号内是构建标识，\`env.get.kernel.build_id\` 返回相同标识。
EOF

echo "==> tar"
tarball="$out/$name.tar.gz"
tar -C "$out" -czf "$tarball" "$name"
rm -rf "$stage"
(cd "$out" && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")

echo "==> done: $tarball"
cat "$tarball.sha256"
