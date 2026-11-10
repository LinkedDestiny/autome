# M-06 · 实现 #6 · B-03 幂等负向对照实证

结论：待审。原版 E5 通过；删除两阶段跳过检查没有破坏本情形的 Git 幂等性；真实推进已并入分支的变异使 E5 失败，还原后通过。未修改产品代码或测试断言，未增加用例。

## 命令、结果与基线

- 原版验收：`cargo test -p automed --test end_to_end a_workspace_merge_left_half_done_finishes_without_a_second_merge_commit > .autome/output/m06-e5.txt 2>&1`，退出 0，1 通过、55 filtered out。相对 M-05/E5 为 1 → 1（差 0）；筛选不是跳过测试层，全套另验。
- 删除跳过检查：`ruby .autome/output/m06-mutate.rb remove-skips > .autome/output/m06-remove-skips.txt 2>&1`，cargo 退出 0，E5 1 通过、55 filtered out。确实移除了 `merge_all` 阶段一与阶段二两处 `is_ancestor` 的 continue；脚本先断言恰好匹配两处。第二次 backend 合并的 stdout 为 `Already up to date.\n`，匹配本任务消息的合并提交数仍为 1；docs 为 1。故 M-04 原先“去掉跳过就出现第二次合并”的推论不成立。
- 有效负向对照（红）：`ruby .autome/output/m06-mutate.rb advance-merged > .autome/output/m06-advance-merged.txt 2>&1`，cargo 退出 101，E5 0 通过、1 失败、55 filtered out。只在 `rebase_all` 发现 rebase 前任务分支已并入 base 且 rebase 成功时，在任务 worktree 执行真实 `git commit --allow-empty`，错误产生一个独有提交。保留两阶段跳过检查，Git 真实再合并 backend，其合并提交数由 1 变 2，docs 为 1。原有 `end_to_end.rs:3578` HEAD 不变断言失败（实际 c1b3fcf0083fa8ae417489d9dff25b772bc52277，预期 5aef6fac67c7958e78dab307e98592f1461c568b）；未改断言、未伪造返回值。
- 临时观测在 `try_merge` 成功后打印 Git stdout 和 `git log <base> --merges --format=%s` 中完全匹配本任务合并消息的数目，不改变判断。首次观测代码因 `std::io::Error` 不能转 `GitError` 编译失败，修正临时观测的错误处理后重跑；该编译失败不算红证据，日志 `.autome/output/m06-instrumentation-compile.txt`。
- 脚本 `ensure` 按原始字节还原 scheduler.rs、git.rs；两次变异后 `git diff -- crates/` 为空。最终不保留任何产品变异或临时观测。

- 还原后的全量：`cargo test --workspace > .autome/output/m06-all.txt 2>&1`，全部通过，总数 **944**，与 M-05 的 944 相比 **差 0**。各二进制为 autome bin 13、cli 4、domain 299、automed lib 555、end_to_end 56、prompt_rendering 10、real_cli 7；0 失败、0 ignored，其余空二进制/doc tests 为 0。没有新增、删除或修改用例，变异只是同一个 E5 的额外运行，不计入总数。
- `cargo fmt --all -- --check > .autome/output/m06-fmt.txt 2>&1` 与 `cargo clippy --all-targets -- -D warnings > .autome/output/m06-clippy.txt 2>&1` 均退出 0。宿主 `sh scripts/ci-local.sh` 留给 Autome 执行，人工 H-01/H-02 未取证。

- 还原后再次原样运行 E5（绿）：`cargo test -p automed --test end_to_end a_workspace_merge_left_half_done_finishes_without_a_second_merge_commit > .autome/output/m06-e5-restored.txt 2>&1`，退出 0，1 通过、55 filtered out。

## 标待审之前的自审清单

1. **情形表逐行**：M-06 无单独情形表，逐项核对其验收契约与 M-04/E5 部分合并路径：原版通过；删除两处跳过实际通过且已记录 Git 返回及数量；真实推进已并入分支实际红；还原绿。全部按真实仓库、真实 worktree、真实 rebase/merge 运行，未人工替代 Git 结果。
2. **失败分支**：E5 首次 docs 主工作树冲突脏文件导致拒绝，backend 已合并，任务回 await_merge，脏文件内容不变；有效变异导致重复合并且原有历史断言红。拒绝授权、读盘失败、超时、空输入、外部服务不可达：不适用，本里程碑只取证部分合并幂等负向对照，未引入这些输入/分支，上一轮没有退回的实现缺陷或要求搬运的检查。
3. **全部迁移**：E5 执行 await_merge → merging →（backend 已合并 / docs 脏被拒）await_merge；清脏并推进 docs main 后，再按合并 → merging → requeued(rebase) → rebase → merging → done（此 Workspace 未配置门禁；配置门禁的路径由既有全量测试覆盖）。E5 检查首次无 merge.rebased、恢复时恰好一次、最终 Done、backend HEAD 不变与两仓各一条合并提交；全量套件覆盖既有转换。本轮无新增状态或通道。
4. **输入域边界**：本域为“已并入 / 未并入”的成员关系与合并次数。E5 用两仓覆盖一个已并入、一个未并入的最小部分合并；原版两仓各 1 条，删除跳过仍各 1 条，有效变异 backend 为 2 条并失败。前导零、闰日、跨年、空串、数值最大值不适用；单仓不能形成部分合并，单仓路径由既有全量测试覆盖。

## 修复说明与可复现变异

本轮修正的是 M-04 负向对照的证据表述，当前产品没有被证明有错，不做无关代码变更。E5 已有的 HEAD/合并数断言足以阻断实测破坏幂等的变异，因此无需新增或修改测试。B-03 保留来源并指向 M-06 证据，关闭权留给审计轮。

将下面脚本保存到 `.autome/output/m06-mutate.rb`，分别运行上面的两个模式即可复现。脚本只对当前 checkout 的产品文件施加临时变异；测试夹具自行建立隔离 Git 仓库，不操作用户主工作树。

```ruby
mode = ARGV.fetch(0)
paths = %w[crates/automed/src/scheduler.rs crates/automed/src/git.rs]
original = paths.to_h { |p| [p, File.binread(p)] }
begin
  s = original.fetch(paths[0]).dup
  if mode == 'remove-skips'
    start = s.index('fn merge_all(')
    before, body = s[0...start], s[start..]
    pattern = /        if git::is_ancestor\(&slot.repo_root, &slot.branch, &slot.base\).unwrap_or\(false\) \{\n(?:            \/\/[^\n]*\n)*            continue;\n        \}\n/
    raise 'expected two skips' unless body.scan(pattern).length == 2
    s = before + body.gsub(pattern, '')
  elsif mode == 'advance-merged'
    needle = '        match git::rebase(&slot.worktree, &slot.base) {'
    replacement = <<'RS'
        let was_merged = git::is_ancestor(&slot.repo_root, &slot.branch, &slot.base).unwrap_or(false);
        match git::rebase(&slot.worktree, &slot.base) {
RS
    raise 'rebase location' unless s.scan(needle).length == 1
    s.sub!(needle, replacement.rstrip)
    s.sub!('            Ok(git::RebaseOutcome::Clean) => {}', <<'RS'.rstrip)
            Ok(git::RebaseOutcome::Clean) => {
                if was_merged {
                    let status = std::process::Command::new("git")
                        .current_dir(&slot.worktree)
                        .args(["-c", "user.name=Mutation", "-c", "user.email=mutation@example.test",
                            "commit", "--allow-empty", "--no-verify", "--no-gpg-sign", "-m", "mutation: advance merged branch"])
                        .status().unwrap();
                    assert!(status.success());
                }
            }
RS
  else
    raise 'unknown mode'
  end
  File.write(paths[0], s)
  g = original.fetch(paths[1]).dup
  needle = "    Ok(MergeAttempt::Merged {\n        commit: head_sha(repo)?,\n    })"
  raise 'merge location' unless g.scan(needle).length == 1
  g.sub!(needle, <<'RS'.rstrip)
    let history = std::process::Command::new("git").current_dir(repo)
        .args(["log", default_branch, "--merges", "--format=%s"]).output().unwrap();
    let history = String::from_utf8_lossy(&history.stdout);
    eprintln!("M06 merge branch={task_branch} stdout={:?} matching_merges={}",
        out.stdout, history.lines().filter(|line| *line == message).count());
    Ok(MergeAttempt::Merged {
        commit: head_sha(repo)?,
    })
RS
  File.write(paths[1], g)
  system('cargo', 'test', '-p', 'automed', '--test', 'end_to_end',
    'a_workspace_merge_left_half_done_finishes_without_a_second_merge_commit', '--', '--nocapture')
  code = $?.exitstatus
  puts "M06 mode=#{mode} cargo_exit=#{code}"
ensure
  original.each { |p, text| File.binwrite(p, text) }
end
exit(code || 1)
```
