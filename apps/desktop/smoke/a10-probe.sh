# 判别检查：账本里有更新的 kernel_mark 时，旧内核启动薄壳，横幅应出现；真实横幅文本如下
export PATH=/usr/bin:/bin; H=$(mktemp -d); export AUTOME_HOME=$H AUTOMED_DB_PATH=$H/state/automed.sqlite3
B=target/debug/autome; $B status >/dev/null; sqlite3 $AUTOMED_DB_PATH "select count(*) from kernel_mark"   # 期望 0：status 不建账本行
sqlite3 $AUTOMED_DB_PATH "insert into kernel_mark(id,build_id,updated_at) values(1,'9999999-20990101T000000Z-ffffffffffff','x')"
# 之后 AUTOME_TEST_BUILD_ID=0000001-... AUTOME_SHELL_SMOKE=1 electron . 得到横幅：
# 「这个内核（…）比推进过账本的内核（…）旧，已停止推进；…」——不含子串「内核比账本旧」
