import { beforeEach, describe, expect, it } from "vitest";
import { useTabsStore } from "./tabs-store";

const W = "C:/worlds/aerth";

describe("tabs store", () => {
  beforeEach(() => useTabsStore.setState({ byWorld: {} }));

  it("opens tabs once and keeps them per world", () => {
    const { openTab } = useTabsStore.getState();
    openTab(W, "a");
    openTab(W, "b");
    openTab(W, "a");
    openTab("other", "z");
    expect(useTabsStore.getState().byWorld[W]).toEqual(["a", "b"]);
    expect(useTabsStore.getState().byWorld["other"]).toEqual(["z"]);
  });

  it("closing returns the right neighbour, then the left, then null", () => {
    const { openTab, closeTab } = useTabsStore.getState();
    openTab(W, "a");
    openTab(W, "b");
    openTab(W, "c");
    expect(closeTab(W, "b")).toBe("c");
    expect(closeTab(W, "c")).toBe("a");
    expect(closeTab(W, "a")).toBeNull();
    expect(closeTab(W, "missing")).toBeNull();
  });
});
