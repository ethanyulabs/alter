#!/bin/sh
# 测试用的假 alterd：写健康文件后等测试建出 $ALTER_HOME/start-logging，然后向 stderr 写 $CHATTY_BYTES 字节
# （默认 256 KB），写完建 $ALTER_HOME/stderr-done。没人读 stderr 时管道写满 64 KB 就卡住，stderr-done 不会出现。
# 收到 SIGTERM / SIGINT 时删除健康文件并退出 0；与真 alterd 一样，stdin（app 持有写端的管道）读到 EOF
# 也按 SIGTERM 退出，app 被强制结束时不留下孤儿进程。
home="${ALTER_HOME:?ALTER_HOME 未设置}"
trap 'rm -f "$home/alterd.json"; exit 0' TERM INT

# 后台列表的 stdin 默认是 /dev/null：先把 stdin 复制到 fd 3 再交给后台读
exec 3<&0
(cat <&3 >/dev/null; kill -TERM $$) &
exec 3<&-

printf '{"version":"0.1.0","pid":%s,"startedAt":%s000,"dbPath":"%s/alter.db"}' "$$" "$(date +%s)" "$home" > "$home/alterd.json"

while [ ! -e "$home/start-logging" ]; do
  sleep 0.05
done
head -c "${CHATTY_BYTES:-262144}" /dev/zero | tr '\0' x >&2
: > "$home/stderr-done"

while :; do
  sleep 1 &
  wait $!
done
