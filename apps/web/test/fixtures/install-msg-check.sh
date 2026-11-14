#!/bin/sh
# 判别检查：macOS /bin/sh(bash 3.2) 安装后 stderr 必须为空，且 stdout 含「已安装面板」与 PATH 提示。用法：a4-install-msg-check.sh <tar.gz>
T=$(mktemp -d); PREFIX=$T/z/bin AUTOME_HOME=$T/h /bin/sh scripts/install-cli.sh --from "$1" >$T/out 2>$T/err
[ ! -s $T/err ] || { echo "FAIL stderr:"; cat $T/err; exit 1; }
grep -q "已安装面板" $T/out && grep -q "不在 PATH" $T/out || { echo "FAIL stdout 缺面板消息/PATH提示"; exit 1; }
echo OK
