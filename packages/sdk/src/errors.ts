// SDK 的统一异常：Gateway 返回的错误一律转成它，code 是 api.md 的 16 个错误码之一
import type { ErrorCode } from "@alter/protocol";

export class AlterError extends Error {
  readonly code: ErrorCode;
  readonly details: Record<string, unknown> | undefined;

  constructor(code: ErrorCode, message: string, details?: Record<string, unknown>) {
    super(`${code}: ${message}`);
    this.name = "AlterError";
    this.code = code;
    this.details = details;
  }
}
