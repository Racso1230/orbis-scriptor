import { beforeEach, describe, expect, it, vi } from "vitest";
import { createQueryClient } from "./query-client";
import { CommandError } from "./unwrap";

const toast = vi.hoisted(() => ({ error: vi.fn(), success: vi.fn() }));
vi.mock("sonner", () => ({ toast }));

const appError = () => new CommandError({ kind: "validation", message: "name is required" });

describe("query client error handling", () => {
  beforeEach(() => vi.clearAllMocks());

  it("toasts a failed mutation with the AppError title and message", async () => {
    const client = createQueryClient();
    await client
      .getMutationCache()
      .build(client, { mutationFn: () => Promise.reject(appError()) })
      .execute(undefined)
      .catch(() => {});
    expect(toast.error).toHaveBeenCalledWith("Invalid input", {
      id: undefined,
      description: "name is required",
    });
  });

  it("does not toast mutations that show their error inline", async () => {
    const client = createQueryClient();
    await client
      .getMutationCache()
      .build(client, {
        mutationFn: () => Promise.reject(appError()),
        meta: { inlineError: true },
      })
      .execute(undefined)
      .catch(() => {});
    expect(toast.error).not.toHaveBeenCalled();
  });

  it("does not toast a failed first load (the component shows it)", async () => {
    const client = createQueryClient();
    await client
      .fetchQuery({ queryKey: ["first"], queryFn: () => Promise.reject(appError()) })
      .catch(() => {});
    expect(toast.error).not.toHaveBeenCalled();
  });

  it("toasts a failed background refetch only once", async () => {
    const client = createQueryClient();
    client.setQueryData(["bg"], "old");
    const failing = () => Promise.reject(appError());
    await client.fetchQuery({ queryKey: ["bg"], queryFn: failing, staleTime: 0 }).catch(() => {});
    await client.fetchQuery({ queryKey: ["bg"], queryFn: failing, staleTime: 0 }).catch(() => {});
    expect(toast.error).toHaveBeenCalledTimes(1);
    expect(toast.error.mock.calls[0]?.[1]).toMatchObject({ id: expect.stringContaining("query-") });
  });
});
