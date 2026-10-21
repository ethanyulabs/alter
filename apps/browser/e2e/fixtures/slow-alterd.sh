#!/bin/sh
# 测试用的假 alterd：3.5 秒后才写健康文件；收到 SIGTERM / SIGINT 时删除健康文件并退出 0。
# sleep 放到后台再 wait，信号到达时能立即处理。
home="${ALTER_HOME:?ALTER_HOME 未设置}"
trap 'rm -f "$home/alterd.json"; exit 0' TERM INT

sleep 3.5 &
wait $!
printf '{"version":"0.1.0","pid":%s,"startedAt":%s000,"dbPath":"%s/alter.db"}' "$$" "$(date +%s)" "$home" > "$home/alterd.json"

while :; do
  sleep 1 &
  wait $!
done
