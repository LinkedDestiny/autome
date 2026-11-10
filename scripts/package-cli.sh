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

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/autome-cli/Cargo.toml | head -1)
[ -n "$version" ] || { echo "package-cli: 读不到 crates/autome-cli/Cargo.toml 的版本号" >&2; exit 1; }
case "$(uname -m)" in
  arm64 | aarch64) arch=aarch64 ;;
  x86_64) arch=x86_64 ;;
  *) echo "package-cli: 不支持的架构 $(uname -m)" >&2; exit 1 ;;
esac
target="$arch-apple-darwin"
name="autome-$version-$target"

echo "==> cargo build --release -p autome-cli"
cargo build --release -p autome-cli

stage="$root/dist/$name"
rm -rf "$stage"
mkdir -p "$stage"
# Copied, never renamed in place: the linker's ad-hoc signature belongs to the
# file it wrote, and a binary rewritten under a running process's feet is one
# macOS kills.
cp "$root/target/release/autome" "$stage/autome"
chmod 755 "$stage/autome"
cat > "$stage/README.md" <<EOF
# autome $version ($target)

Autome 的命令行：一句话需求，经设计、评审、实现、审计，到合并。

安装：把 \`autome\` 放到 PATH 里任意目录，例如

    install -m 755 autome ~/.local/bin/autome

或者在解包目录外运行仓库里的 \`scripts/install-cli.sh --from <本 tar 包>\`。

然后：

    autome doctor                    # Git、Claude Code、Codex 与登录状态
    autome project add ~/code/shop --skip-onboarding
    cd ~/code/shop && autome task new "购物车加一个结算按钮"
    autome run                       # 驱动 Loop，到卡点会提示你下一步
    autome help

状态在 \$AUTOME_HOME（默认 ~/.autome），与桌面端共用。
EOF

echo "==> tar"
tarball="$root/dist/$name.tar.gz"
tar -C "$root/dist" -czf "$tarball" "$name"
rm -rf "$stage"
(cd "$root/dist" && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")

echo "==> done: $tarball"
cat "$tarball.sha256"
