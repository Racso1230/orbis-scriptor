import { beforeEach, describe, expect, it } from "vitest";
import { useMapToolStore } from "./map-tool-store";

describe("map tool store", () => {
  beforeEach(() =>
    useMapToolStore.setState({
      tool: "select",
      selectedObjectId: null,
      pendingFocus: null,
      viewports: {},
      activeLayerIds: {},
      pathPreset: "river",
    }),
  );

  it("clears the selection when the tool changes", () => {
    const s = useMapToolStore.getState();
    s.select("pin-1");
    expect(useMapToolStore.getState().selectedObjectId).toBe("pin-1");
    s.setTool("pin");
    expect(useMapToolStore.getState()).toMatchObject({ tool: "pin", selectedObjectId: null });
  });

  it("stores and clears a pending focus", () => {
    const s = useMapToolStore.getState();
    s.setPendingFocus({ mapEntityId: "m", objectId: "o" });
    expect(useMapToolStore.getState().pendingFocus).toEqual({ mapEntityId: "m", objectId: "o" });
    s.setPendingFocus(null);
    expect(useMapToolStore.getState().pendingFocus).toBeNull();
  });

  it("remembers a viewport per map", () => {
    const s = useMapToolStore.getState();
    s.saveViewport("a", { scale: 2, x: 1, y: 2 });
    s.saveViewport("b", { scale: 3, x: 0, y: 0 });
    expect(useMapToolStore.getState().viewports["a"]).toEqual({ scale: 2, x: 1, y: 2 });
  });

  it("remembers the active layer per map", () => {
    const s = useMapToolStore.getState();
    s.setActiveLayer("a", "layer-1");
    s.setActiveLayer("b", "layer-9");
    expect(useMapToolStore.getState().activeLayerIds).toEqual({ a: "layer-1", b: "layer-9" });
  });

  it("switches to a drawing tool and keeps the path preset", () => {
    const s = useMapToolStore.getState();
    s.setPathPreset("road");
    s.setTool("path");
    expect(useMapToolStore.getState()).toMatchObject({ tool: "path", pathPreset: "road" });
  });

  it("closes the label text box when the tool changes", () => {
    const s = useMapToolStore.getState();
    s.setLabelInput({ mode: "create", x: 1, y: 2, pathId: null, layerId: "l" });
    s.setTool("stamp");
    expect(useMapToolStore.getState()).toMatchObject({ tool: "stamp", labelInput: null });
  });

  it("remembers the chosen stamp", () => {
    useMapToolStore.getState().setStampIcon("castle");
    expect(useMapToolStore.getState().stampIcon).toBe("castle");
  });
});
