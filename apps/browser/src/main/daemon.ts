// 托管 alterd：解析可执行文件路径、启动子进程、轮询健康文件判断是否就绪
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import type { DaemonStatus } from "../shared/daemon";
import { forwardToStderr } from "./stdio";

/** 数据目录：ALTER_HOME，未设置时为 ~/Library/Application Support/alter（与 alter-core 一致）。 */
export function alterHome(): string {
  const env = process.env["ALTER_HOME"];
  return env ? env : join(homedir(), "Library/Application Support/alter");
}

/**
 * alterd 可执行文件路径，找不到时返回 null。
 * 设置了 ALTER_ALTERD_PATH 就只认它；否则依次找打包态的 resources/bin/alterd 与仓库内的 target/debug/alterd。
 */
export function resolveAlterdPath(): string | null {
  const override = process.env["ALTER_ALTERD_PATH"];
  if (override) {
    return existsSync(override) ? override : null;
  }
  const candidates = [
    join(process.resourcesPath, "bin/alterd"),
    // 开发态：out/main → apps/browser → 仓库根
    join(__dirname, "../../../../target/debug/alterd"),
  ];
  return candidates.find((p) => existsSync(p)) ?? null;
}

/** 读健康文件里的 pid；文件不存在或内容不完整时返回 undefined。 */
function healthPid(home: string): number | undefined {
  try {
    const pid = JSON.parse(readFileSync(join(home, "alterd.json"), "utf8")).pid;
    return typeof pid === "number" ? pid : undefined;
  } catch {
    return undefined;
  }
}

/** pid 对应的进程是否存活；EPERM 说明进程在但无权发信号，也算存活（与 alter-core 的 health::is_alive 一致）。 */
function isAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return (e as NodeJS.ErrnoException).code === "EPERM";
  }
}

/** alterd 发现已有实例在运行时的退出码（见 api.md「进程状态」）。 */
const EXIT_ALREADY_RUNNING = 3;
const POLL_INTERVAL_MS = 100;
/** 发出 SIGTERM 后等待 alterd 自行退出的上限，超时改用 SIGKILL。 */
const STOP_TIMEOUT_MS = 2_000;

export class DaemonSupervisor {
  private child: ChildProcess | undefined;
  private status: DaemonStatus = { state: "starting" };
  private stopping = false;

  constructor(private readonly onStatus: (status: DaemonStatus) => void) {}

  current(): DaemonStatus {
    return this.status;
  }

  start(): void {
    const path = resolveAlterdPath();
    if (path === null) {
      this.setStatus({ state: "missing", path: process.env["ALTER_ALTERD_PATH"] });
      return;
    }
    const home = alterHome();
    // stderr 用管道接过来再转发，而不是继承：alterd 不能持有主进程自己的 stdio，否则会拖住主进程的退出。
    // 转发要一直读下去，主进程的 stderr 断开了也不停（见 forwardToStderr），否则 alterd 写满管道就卡住。
    // stdin 是一根只由本进程持有写端的管道：主进程结束（包括被 SIGKILL）时内核关闭写端，
    // alterd 读到 EOF 后自行退出。运行期间不要对 child.stdin 调用 end()。
    const child = spawn(path, ["run", "--exit-on-stdin-close"], {
      stdio: ["pipe", "ignore", "pipe"],
    });
    if (child.stderr) forwardToStderr(child.stderr);
    this.child = child;
    this.setStatus({ state: "starting", path });
    // 文件存在但无法执行（权限等）时 spawn 以 error 事件报告，不能让它变成主进程的未捕获异常
    child.on("error", () => this.setStatus({ state: "missing", path }));
    child.on("exit", (code) => {
      if (this.stopping) return;
      // 退出码 3：同一 ALTER_HOME 已有 alterd 在运行（例如 app 之外手动拉起的），复用它。
      // 它不是本进程拉起的，stop() 不会去停它：这个子进程已经退出，stop() 直接返回。
      const existing = code === EXIT_ALREADY_RUNNING ? healthPid(home) : undefined;
      if (existing !== undefined && isAlive(existing)) {
        this.setStatus({ state: "running", pid: existing, path });
      } else {
        this.setStatus({ state: "exited", path });
      }
    });

    // 一直轮询到看见健康文件或子进程退出：启动慢（比如以后的大迁移）也不会卡在「启动中」
    const timer = setInterval(() => {
      if (child.exitCode !== null || child.signalCode !== null) {
        clearInterval(timer);
      } else if (child.pid !== undefined && healthPid(home) === child.pid) {
        clearInterval(timer);
        this.setStatus({ state: "running", pid: child.pid, path });
      }
    }, POLL_INTERVAL_MS);
  }

  /** 关闭 alterd：SIGTERM，2 秒内没退出就 SIGKILL 并替它删除健康文件。 */
  stop(): Promise<void> {
    const child = this.child;
    // 没有 pid 说明 spawn 本身失败了，不会再有 exit 事件，不能等
    if (!child || child.pid === undefined || child.exitCode !== null || child.signalCode !== null) {
      return Promise.resolve();
    }
    this.stopping = true;
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        child.kill("SIGKILL");
        if (healthPid(alterHome()) === child.pid) {
          rmSync(join(alterHome(), "alterd.json"), { force: true });
        }
      }, STOP_TIMEOUT_MS);
      child.once("exit", () => {
        clearTimeout(timer);
        resolve();
      });
      child.kill("SIGTERM");
    });
  }

  private setStatus(status: DaemonStatus): void {
    this.status = status;
    this.onStatus(status);
  }
}
