# Sourced by run.sh. These checks enforce the published skill contract.
skill="$plugin/skills/autome-task/SKILL.md"
section() { awk -v h="## $1" '$0==h {on=1;next} /^## / && on {exit} on' "$skill"; }
has() { grep -Fq -- "$1"; }
T1() {
  [ "$(grep '^## ' "$skill")" = "$(printf '## %s\n' '1. 整理与追问' '2. 检查命令行' '3. 确定项目' '4. 展示并等待确认' '5. 创建' '6. 回显' '失败时' '禁止')" ]
}
T2() { for word in '没有明确确认，不执行第 5 步' 差不多 再看看 再次完整展示 再次等待确认; do section '4. 展示并等待确认' | has "$word" || return 1; done; }
T3() {
  [ "$(sed -n '/^<!-- create-command -->$/,/^<!-- \/create-command -->$/p' "$skill" | sed '1d;$d')" = "autome task new --brief - --kind {{KIND}} --draft -p {{PROJECT_ID}} --json < '{{CARD_FILE}}'" ] && ! grep -Fq '<<' "$skill"
}
T4() {
  for word in 直接开始 创建并开始 建好就开始 不要待开始 马上开始跑; do section '4. 展示并等待确认' | has "$word" || return 1; done
  for word in '创建后不调用 `autome task start`' '1. `queue_position`' '2. 否则' '3. 否则' '4. 以上都不满足'; do section '6. 回显' | has "$word" || return 1; done
  # The queue branch must not suggest a start command, even as prose.
  ! section '6. 回显' | sed -n '/^1\./p' | has 'autome task start'
}
T5() { for word in 'autome rpc' task.create sqlite3; do section 禁止 | has "$word" || return 1; sed '/^## 禁止/,$d' "$skill" | has "$word" && return 1; done; return 0; }
T6() { ! sed '/^## 6\./,$d' "$skill" | has 'autome task start'; }
T7() { for word in 'command -v autome' 'sh scripts/install-cli.sh' 'curl -fsSL https://raw.githubusercontent.com/LinkedDestiny/autome/main/scripts/install-cli.sh | sh'; do section '2. 检查命令行' | has "$word" || return 1; done; }
T8() { for word in 'autome project ls --json' 'pwd -P' removed_at '-p' 'path + "/"' 最长; do section '3. 确定项目' | has "$word" || return 1; done; }
T9() { for name in check-card.sh panel-url.sh; do has "$name" < "$skill" && [ -f "$plugin/skills/autome-task/scripts/$name" ] || return 1; done; }
T10() { grep '^description:' "$skill" | has '建成 autome 任务'; }
T11() {
  # All environment commands must occur after CLI discovery, in step 2 only.
  awk '
    /^## / { step=$0 }
    /git rev-parse|autome version/ {
      if (step!="## 2. 检查命令行" || !discovered) bad=1
    }
    /command -v autome/ && step=="## 2. 检查命令行" { discovered=1 }
    END { exit bad }
  ' "$skill" || return 1
  for command in 'git rev-parse --abbrev-ref HEAD' 'git rev-parse --short HEAD' 'autome version'; do
    section '2. 检查命令行' | has "$command" || return 1
  done
}
T12() {
  for word in '不跑命令' '只有 Bug 的「环境」缺失可留待第 2 步' '其余必填信息缺失仍先追问'; do
    section '1. 整理与追问' | has "$word" || return 1
  done
}
T13() {
  for word in '确认命令行存在后' '环境缺失时' '用户明确同意后才写入' '不同意或只作模糊回应' '采集失败或结果为空' '回第 1 步追问' '不等于确认创建'; do
    section '2. 检查命令行' | has "$word" || return 1
  done
}
