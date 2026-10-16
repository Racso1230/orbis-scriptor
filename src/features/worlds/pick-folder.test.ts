import { describe, expect, it } from "vitest";
import { folderName } from "./pick-folder";

describe("folderName", () => {
  it("returns the last segment for Windows and POSIX paths", () => {
    expect(folderName("C:\\Users\\me\\Worlds\\Aerth")).toBe("Aerth");
    expect(folderName("/home/me/worlds/aerth/")).toBe("aerth");
  });

  it("returns an empty string for an empty path", () => {
    expect(folderName("")).toBe("");
  });
});
