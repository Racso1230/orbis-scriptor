import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithQuery } from "@/test/render";
import { TimelinePage } from "./timeline-view";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  getDefaultCalendar: vi.fn(),
  listEvents: vi.fn(),
  devSeedEvents: vi.fn(),
}));

vi.mock("@/lib/bindings", () => ({ commands }));
vi.mock("@/features/entities/use-open-entity", () => ({ useOpenEntity: () => vi.fn() }));

// Smoke test only: pan/zoom/drag are covered by the pure scale/ticks/lanes tests.
describe("TimelinePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.getDefaultCalendar.mockResolvedValue(
      ok({
        id: "cal",
        name: "Default",
        isDefault: true,
        definition: {
          months: Array.from({ length: 12 }, (_, i) => ({ name: `Month ${i + 1}`, days: 30 })),
          weekdays: [],
          eras: [{ name: "After Founding", abbreviation: "AF", startDay: 0 }],
        },
      }),
    );
    commands.listEvents.mockResolvedValue(
      ok([
        {
          entityId: "e1",
          name: "The Great War",
          startDay: -200,
          endDay: 400,
          importance: 5,
          calendarId: "cal",
        },
        {
          entityId: "e2",
          name: "Coronation",
          startDay: 50,
          endDay: null,
          importance: 3,
          calendarId: "cal",
        },
      ]),
    );
  });

  it("renders events as labelled shapes and requests a padded window", async () => {
    renderWithQuery(<TimelinePage />);

    expect(await screen.findByRole("button", { name: "The Great War" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Coronation" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /fit all events/i })).toBeInTheDocument();

    const [from, to] = commands.listEvents.mock.calls[0] as [number, number];
    expect(from).toBeLessThan(0);
    expect(to).toBeGreaterThan(0);
  });
});
