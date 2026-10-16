import { describe, expect, it } from "vitest";
import { CommandError, unwrap } from "./unwrap";

describe("unwrap", () => {
  it("returns the data of an ok result", async () => {
    await expect(unwrap(Promise.resolve({ status: "ok", data: 42 }))).resolves.toBe(42);
  });

  it("throws a CommandError carrying kind and message for an error result", async () => {
    const promise = unwrap(
      Promise.resolve({
        status: "error",
        error: { kind: "notFound", message: "entity 42" },
      }),
    );

    await expect(promise).rejects.toBeInstanceOf(CommandError);
    await expect(promise).rejects.toMatchObject({ kind: "notFound", message: "entity 42" });
  });
});
