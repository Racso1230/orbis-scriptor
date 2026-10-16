import type { AppError } from "@/lib/bindings";

/** The shape tauri-specta wraps every command result in. */
type SpectaResult<T, E> = { status: "ok"; data: T } | { status: "error"; error: E };

/** An `AppError` from Rust, re-thrown as a JS Error so TanStack Query sees a failure. */
export class CommandError extends Error {
  readonly kind: AppError["kind"];

  constructor(readonly error: AppError) {
    super(error.message);
    this.name = "CommandError";
    this.kind = error.kind;
  }
}

/**
 * Turns a tauri-specta result into a plain value or a thrown `CommandError`.
 * Every query/mutation hook wraps its command call in this.
 */
export async function unwrap<T>(result: Promise<SpectaResult<T, AppError>>): Promise<T> {
  const r = await result;
  if (r.status === "ok") return r.data;
  throw new CommandError(r.error);
}
