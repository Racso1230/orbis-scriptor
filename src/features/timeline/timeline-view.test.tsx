import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ELVEN, STANDARD } from "@/features/calendars/test-calendars";
import { renderWithQuery } from "@/test/render";
import { formatDate } from "./calendar";
import { TimelinePage } from "./timeline-view";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  getDisplayCalendar: vi.fn(),
  listCalendars: vi.fn(),
  setDisplayCalendar: vi.fn(),
  listEvents: vi.fn(),
  devSeedEvents: vi.fn(),
}));

vi.mock("@/lib/bindings", () => ({ commands }));
const navigate = vi.hoisted(() => vi.fn());
vi.mock("@tanstack/react-router", () => ({ useNavigate: () => navigate }));
vi.mock("@/features/entities/use-open-entity", () => ({ useOpenEntity: () => vi.fn() }));

// Smoke test only: pan/zoom/drag are covered by the pure scale/ticks/lanes tests.
describe("TimelinePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.getDisplayCalendar.mockResolvedValue(ok(STANDARD));
    commands.listCalendars.mockResolvedValue(ok([STANDARD, ELVEN]));
    commands.setDisplayCalendar.mockResolvedValue(ok(ELVEN));
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

  it("switches the display calendar and redraws the centre date in it", async () => {
    renderWithQuery(<TimelinePage />);

    const centre = await screen.findByLabelText("Centre of the view");
    // The default viewport is centred on day 0.
    expect(centre).toHaveTextContent(formatDate(STANDARD.definition, 0, "monthYear"));

    fireEvent.change(await screen.findByRole("combobox", { name: "Display calendar" }), {
      target: { value: ELVEN.id },
    });

    await waitFor(() =>
      expect(screen.getByLabelText("Centre of the view")).toHaveTextContent(
        formatDate(ELVEN.definition, 0, "monthYear"),
      ),
    );
    expect(commands.setDisplayCalendar).toHaveBeenCalledWith(ELVEN.id);
    expect(formatDate(ELVEN.definition, 0, "monthYear")).not.toBe(
      formatDate(STANDARD.definition, 0, "monthYear"),
    );
  });

  it("opens the calendar manager from the switcher", async () => {
    renderWithQuery(<TimelinePage />);
    await screen.findByLabelText("Centre of the view");
    fireEvent.change(await screen.findByRole("combobox", { name: "Display calendar" }), {
      target: { value: "__manage__" },
    });
    expect(navigate).toHaveBeenCalledWith({ to: "/world/calendars" });
    expect(commands.setDisplayCalendar).not.toHaveBeenCalled();
  });
});
