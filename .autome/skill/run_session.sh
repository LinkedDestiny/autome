#!/bin/sh
# autome-scaffold-version: 7
#
# Autome 会话包装脚本。由 Autome 在可见终端中调用，不要手工修改
# （下次初始化会按版本号刷新；删掉上面的版本标记行即可接管本文件）。
#
# 用法：
#   run_session.sh <session-id> <output-dir> <runtime> <renderer> <binary> <prompt-file> [extra-args...]
#
# <renderer> 是 automed 自己的可执行文件路径，用来把 CLI 的 JSONL 输出渲染成
# 人能读的日志；传 `-` 或传一个不可执行的路径即跳过渲染。
#
# 职责：写 pid → 执行 CLI 并把输出 tee 到日志 → 无论如何都写退出标记。

set -u

if [ $# -lt 6 ]; then
  echo "run_session.sh: 参数不足" >&2
  exit 64
fi

session_id="$1"; shift
out_dir="$1"; shift
runtime="$1"; shift
renderer="$1"; shift
binary="$1"; shift
prompt_file="$1"; shift

log="$out_dir/$session_id.log"
pid_file="$out_dir/$session_id.pid"
exit_file="$out_dir/$session_id.exit"

# If the output directory cannot be created there is nowhere to write the log,
# the pid or the marker, and every later line would fail in turn — five
# cascading errors that say nothing about the cause. Stop here instead.
if ! mkdir -p "$out_dir" 2>/dev/null || [ ! -d "$out_dir" ]; then
  echo "run_session.sh: 会话目录不存在且无法创建：$out_dir" >&2
  echo "run_session.sh: 项目可能已被移动或删除。" >&2
  exit 65
fi
if ! : > "$log" 2>/dev/null; then
  echo "run_session.sh: 无法写入日志 $log" >&2
  echo "run_session.sh: 项目可能已被移动或删除。" >&2
  exit 65
fi

# 退出标记：无论正常结束、被 kill 还是脚本出错都要写出。
# 先写到临时文件再 mv，保证读取方不会看到写了一半的标记。
write_marker() {
  code=$1
  tmp="$exit_file.tmp"
  {
    echo "exit_code=$code"
    echo "ended_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "marker_end"
  } > "$tmp" 2>/dev/null
  mv -f "$tmp" "$exit_file" 2>/dev/null
}

on_signal() {
  write_marker 130
  exit 130
}
trap on_signal INT TERM HUP

echo $$ > "$pid_file"

printf '=== autome session %s (%s) started %s ===\n' \
  "$session_id" "$runtime" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$log"

if [ ! -f "$prompt_file" ]; then
  echo "run_session.sh: 找不到 prompt 文件 $prompt_file" >> "$log"
  write_marker 66
  exit 66
fi

# CLI 的 stdout/stderr 同时进日志和终端，用户能实时看到，Autome 事后能读到。
#
# 退出码必须取 CLI 的，不是 tee 的。POSIX sh 没有 bash 的管道状态数组，
# 所以把 CLI 的退出码写进一个临时文件再读回来——这是可移植的写法。
# 这里错了的后果是：崩溃的会话会被当成正常结束，内核会去解析一份不存在或半截的
# 设计文档，然后把协议失败归错到文档头上。
code_file="$out_dir/$session_id.code"
rm -f "$code_file"

# 两个 CLI 都用 JSONL 跑：Claude 是 --output-format stream-json，Codex 是 --json。
#
# Claude 的 text 模式只打印收尾那段总结——会话不开窗口，日志是唯一能看到这一轮
# 干了什么的地方，而 text 模式下它只有几 KB。Codex 的默认输出本来就是给人看的，
# 但它不带用量：费用、turn 数、token 全都拿不到，内核记不下任何东西。
#
# JSONL 是完整的，但一行一个 JSON，人读不了，所以经 automed 自己渲染一遍。
# 原始流另存 .jsonl——内核事后从它读用量，渲染只影响展示、不丢东西。
if [ -n "$renderer" ] && [ "$renderer" != "-" ] && [ -x "$renderer" ]; then
  { "$binary" "$@" < "$prompt_file" 2>&1; echo $? > "$code_file"; } \
    | tee -a "$out_dir/$session_id.jsonl" \
    | "$renderer" render-stream "$runtime" \
    | tee -a "$log"
else
  { "$binary" "$@" < "$prompt_file" 2>&1; echo $? > "$code_file"; } | tee -a "$log"
fi
code=$(cat "$code_file" 2>/dev/null || echo 70)
rm -f "$code_file"

printf '=== autome session %s finished, exit %s ===\n' "$session_id" "$code" >> "$log"
write_marker "$code"
rm -f "$pid_file"
exit "$code"
