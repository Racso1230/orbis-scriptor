import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { GenerateFinished, GenerateProgress } from "@/lib/bindings";
import { renderWithQuery } from "@/test/render";
import { GENERATE_IDLE } from "./generate-state";
import { useGenerateStore } from "./generate-store";
import { GenerateWizard } from "./generate-wizard";

type Handler = (e: { payload: unknown }) => void;

const h = vi.hoisted(() => ({
  previewGeneration: vi.fn(),
  generateMap: vi.fn(),
  progress: null as null | Handler,
  finished: null as null | Handler,
  openEntity: vi.fn(),
}));

vi.mock("@/lib/bindings", () => ({
  commands: { previewGeneration: h.previewGeneration, generateMap: h.generateMap },
  events: {
    generateProgress: {
      listen: (fn: Handler) => {
        h.progress = fn;
        return Promise.resolve(() => {});
      },
    },
    generateFinished: {
      listen: (fn: Handler) => {
        h.finished = fn;
        return Promise.resolve(() => {});
      },
    },
  },
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
vi.mock("@/lib/query-client", () => ({ queryClient: { invalidateQueries: vi.fn() } }));
vi.mock("@/features/entities/use-open-entity", () => ({ useOpenEntity: () => h.openEntity }));

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

function openWizard() {
  renderWithQuery(<GenerateWizard />);
  act(() => useGenerateStore.getState().openDialog());
}

const seedInput = () => screen.getByLabelText<HTMLInputElement>("Seed");

describe("GenerateWizard", () => {
  beforeAll(() => {
    // Radix sliders measure themselves; jsdom has no ResizeObserver.
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  });

  beforeEach(() => {
    vi.clearAllMocks();
    h.progress = null;
    h.finished = null;
    useGenerateStore.setState({
      state: GENERATE_IDLE,
      dialogOpen: false,
      openCount: 0,
      lastParams: null,
    });
    h.previewGeneration.mockResolvedValue(ok("AAAA"));
    h.generateMap.mockResolvedValue(ok("job-1"));
  });

  it("shows the default settings and renders a preview", async () => {
    openWizard();
    expect(await screen.findByRole("heading", { name: "Generate map" })).toBeInTheDocument();
    expect(seedInput().value).toMatch(/^\d+$/);
    expect(screen.getByLabelText<HTMLSelectElement>("Map size").value).toBe("medium");
    expect(screen.getByLabelText<HTMLSelectElement>("Land shape").value).toBe("continent");
    expect(screen.getByLabelText<HTMLSelectElement>("Climate").value).toBe("temperate");
    expect(screen.getByLabelText("Kingdoms")).toBeDisabled();
    expect(screen.getByLabelText("Scope")).toBeDisabled();
    expect(screen.getByText("Places and history arrive in 14b/14c.")).toBeInTheDocument();

    const img = await screen.findByRole<HTMLImageElement>("img", { name: "Terrain preview" });
    expect(img.src).toBe("data:image/png;base64,AAAA");
    expect(h.previewGeneration).toHaveBeenCalledWith(
      expect.objectContaining({ seed: Number(seedInput().value), width: 4000, height: 3000 }),
      384,
    );
  });

  it("rerolls the seed and re-renders the preview", async () => {
    openWizard();
    await screen.findByRole("img", { name: "Terrain preview" });
    const before = seedInput().value;
    fireEvent.click(screen.getByRole("button", { name: /Reroll/ }));
    expect(seedInput().value).not.toBe(before);
    await waitFor(() =>
      expect(h.previewGeneration).toHaveBeenLastCalledWith(
        expect.objectContaining({ seed: Number(seedInput().value) }),
        384,
      ),
    );
  });

  it("blocks Generate while the settings are invalid", async () => {
    openWizard();
    fireEvent.change(await screen.findByLabelText("Seed"), { target: { value: "abc" } });
    expect(screen.getByRole("alert")).toHaveTextContent(/Seed must be a whole number/);
    expect(screen.getByRole("button", { name: "Generate" })).toBeDisabled();

    fireEvent.change(seedInput(), { target: { value: "1" } });
    fireEvent.change(screen.getByLabelText("Map size"), { target: { value: "custom" } });
    fireEvent.change(screen.getByLabelText("Width (px)"), { target: { value: "100" } });
    expect(screen.getByRole("alert")).toHaveTextContent(/Map size must be between/);
    expect(screen.getByRole("button", { name: "Generate" })).toBeDisabled();
  });

  it("generates with the chosen settings, shows progress and the result", async () => {
    openWizard();
    fireEvent.change(await screen.findByLabelText("Seed"), { target: { value: "42" } });
    fireEvent.change(screen.getByLabelText("Land shape"), { target: { value: "archipelago" } });
    fireEvent.change(screen.getByLabelText("Climate"), { target: { value: "frozen" } });
    fireEvent.change(screen.getByLabelText("Map size"), { target: { value: "small" } });
    fireEvent.change(screen.getByLabelText("Map name"), { target: { value: "Isles" } });
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() =>
      expect(h.generateMap).toHaveBeenCalledWith(
        {
          seed: 42,
          width: 2000,
          height: 1500,
          landShape: "archipelago",
          climate: "frozen",
          landPercent: 40,
          temperature: 0,
          rainfall: 0,
          kingdoms: 5,
          scope: "mapOnly",
        },
        "Isles",
      ),
    );
    await waitFor(() => expect(useGenerateStore.getState().state.jobId).toBe("job-1"));

    const p: GenerateProgress = { jobId: "job-1", phase: "writing", done: 2, total: 5 };
    act(() => h.progress?.({ payload: p }));
    expect(screen.getByRole("progressbar", { name: "Generation progress" })).toBeInTheDocument();
    expect(screen.getByText(/Writing the map to the world… 2 \/ 5/)).toBeInTheDocument();

    const f: GenerateFinished = {
      jobId: "job-1",
      ok: true,
      mapEntityId: "map-9",
      stats: { landPercentActual: 38.5, landMasses: 11, rivers: 7, stamps: 120, biomeCounts: {} },
      error: null,
    };
    act(() => h.finished?.({ payload: f }));
    const result = screen.getByRole("region", { name: "Generation result" });
    expect(result).toHaveTextContent("Generated a map with 11 land masses.");
    expect(result).toHaveTextContent("38.5 %");
    expect(result).toHaveTextContent("120");

    fireEvent.click(screen.getByRole("button", { name: "Open map" }));
    expect(h.openEntity).toHaveBeenCalledWith("map-9");
    expect(useGenerateStore.getState().dialogOpen).toBe(false);
  });

  it("shows the error of a failed generation", async () => {
    openWizard();
    fireEvent.click(await screen.findByRole("button", { name: "Generate" }));
    await waitFor(() => expect(useGenerateStore.getState().state.jobId).toBe("job-1"));
    act(() =>
      h.finished?.({
        payload: {
          jobId: "job-1",
          ok: false,
          mapEntityId: null,
          stats: null,
          error: "Disk full",
        } satisfies GenerateFinished,
      }),
    );
    expect(screen.getByRole("alert")).toHaveTextContent("Disk full");
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByRole("button", { name: "Generate" })).toBeInTheDocument();
  });
});
