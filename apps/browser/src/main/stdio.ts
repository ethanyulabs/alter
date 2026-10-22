// 主进程的 stdout / stderr：读端可能已经关闭（启动 app 的终端或测试进程已经退出、管道被关掉），
// 之后的写入会得到 EPIPE。日志此时本来就无处可写，也没有别处可报告，写入错误一律忽略，不让它成为未捕获异常。

/** stderr 是否已经写不出去（出过错）。stdio 流出错后仍是 `writable`、`errored` 也不置位，只能自己记。 */
let stderrBroken = false;

/** stderr 断开时要执行的回调（恢复被背压暂停的转发）。 */
const onStderrBroken: (() => void)[] = [];

/** 给 `process.stdout`、`process.stderr` 加 `error` 监听。要在任何日志输出之前调用。 */
export function guardStdio(): void {
  process.stdout.on("error", () => {});
  process.stderr.on("error", () => {
    if (stderrBroken) return;
    stderrBroken = true;
    for (const f of onStderrBroken.splice(0)) f();
  });
}

/**
 * 把子进程的输出转发到主进程的 stderr（要先调用过 `guardStdio`）。不用 `pipe`：`pipe` 在 stderr 出错时自动解除，
 * 之后没人读子进程的输出，它写满管道（64 KB）就会卡住。
 * - stderr 正常时保留背压：写入返回 false 就暂停读取，`drain` 后恢复，读得慢时不在主进程里堆积。
 * - stderr 断开后不再暂停：恢复读取，数据直接丢弃。
 */
export function forwardToStderr(stream: NodeJS.ReadableStream): void {
  onStderrBroken.push(() => stream.resume());
  stream.on("data", (chunk: Buffer | string) => {
    if (stderrBroken) return;
    if (!process.stderr.write(chunk)) {
      stream.pause();
      process.stderr.once("drain", () => stream.resume());
    }
  });
}
