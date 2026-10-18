import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CalendarDefinition } from "@/lib/bindings";
import { renderWithQuery } from "@/test/render";
import { CalendarEditor } from "./calendar-editor";
import { ELVEN, STANDARD } from "./test-calendars";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  previewCalendar: vi.fn(),
  createCalendar: vi.fn(),
  updateCalendar: vi.fn(),
}));
vi.mock("@/lib/bindings", () => ({ commands }));

/** The definition the preview was last asked about. */
function lastPreviewed(): CalendarDefinition {
  const call = commands.previewCalendar.mock.calls.at(-1) as [CalendarDefinition, number[], string];
  return call[0];
}

describe("CalendarEditor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // A valid preview: one label per requested day.
    commands.previewCalendar.mockImplementation((_def: CalendarDefinition, days: number[]) =>
      Promise.resolve({ issues: [], labels: days.map((d) => `label for day ${d}`) }),
    );
    commands.createCalendar.mockImplementation((name: string, definition: CalendarDefinition) =>
      Promise.resolve(ok({ id: "new-id", name, definition, isDefault: false })),
    );
    commands.updateCalendar.mockImplementation(
      (id: string, name: string, definition: CalendarDefinition) =>
        Promise.resolve(ok({ id, name, definition, isDefault: false })),
    );
  });

  it("previews the unsaved definition after an edit (add a month)", async () => {
    renderWithQuery(
      <CalendarEditor calendar={STANDARD} onSaved={vi.fn()} onDirtyChange={vi.fn()} />,
    );
    await waitFor(() => expect(commands.previewCalendar).toHaveBeenCalled());
    expect(lastPreviewed().months).toHaveLength(12);

    fireEvent.click(screen.getByRole("button", { name: "Add month" }));
    expect(screen.getByLabelText("Month 13 name")).toBeInTheDocument();

    // The preview is debounced, then asked about the 13-month draft.
    await waitFor(() => expect(lastPreviewed().months).toHaveLength(13));
    expect(lastPreviewed().months.at(-1)).toEqual({ name: "Month 13", days: 30 });
    // Eras' start days, then day 0, the timeline centre and spaced days are formatted.
    const days = commands.previewCalendar.mock.calls.at(-1)?.[1] as number[];
    expect(days[0]).toBe(0); // the one era starts on day 0
    expect(days.length).toBe(1 + 2 + 4);
    await waitFor(() =>
      expect(screen.getByRole("complementary", { name: "Preview" })).toHaveTextContent(
        "Normal year: 390 days",
      ),
    );
    expect(await screen.findAllByText(/label for day/)).not.toHaveLength(0);
  });

  it("shows the backend's validation issues next to the field", async () => {
    commands.previewCalendar.mockResolvedValue({
      issues: [{ field: "months[0].days", message: "must be at least 1" }],
      labels: [],
    });
    renderWithQuery(
      <CalendarEditor calendar={STANDARD} onSaved={vi.fn()} onDirtyChange={vi.fn()} />,
    );

    const error = await screen.findByText("must be at least 1", { selector: "p#err-month-days-0" });
    expect(error).toBeInTheDocument();
    expect(screen.getByLabelText("Month 1 days")).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByLabelText("Month 2 days")).not.toHaveAttribute("aria-invalid");
    // The preview panel lists the same problem instead of dates.
    expect(screen.getByText(/Fix these to see the preview/)).toBeInTheDocument();
  });

  it("flags unreadable numbers locally and does not call the backend for them", async () => {
    renderWithQuery(
      <CalendarEditor calendar={STANDARD} onSaved={vi.fn()} onDirtyChange={vi.fn()} />,
    );
    await waitFor(() => expect(commands.previewCalendar).toHaveBeenCalled());

    fireEvent.change(screen.getByLabelText("Month 3 days"), { target: { value: "" } });
    expect(screen.getAllByText("Enter a whole number.").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Save changes" })).toBeDisabled();
  });

  it("creates a new calendar with the edited name and definition", async () => {
    const onSaved = vi.fn();
    const onDirty = vi.fn();
    renderWithQuery(<CalendarEditor calendar={null} onSaved={onSaved} onDirtyChange={onDirty} />);

    // A new calendar is not dirty until something changes, so it cannot be created as-is.
    expect(screen.getByRole("button", { name: "Create calendar" })).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Calendar name"), { target: { value: "Dwarven" } });
    expect(onDirty).toHaveBeenLastCalledWith(true);
    fireEvent.click(screen.getByRole("button", { name: "Add month" }));
    fireEvent.click(screen.getByRole("button", { name: "Create calendar" }));

    await waitFor(() => expect(commands.createCalendar).toHaveBeenCalledTimes(1));
    const [name, definition] = commands.createCalendar.mock.calls[0] as [
      string,
      CalendarDefinition,
    ];
    expect(name).toBe("Dwarven");
    expect(definition.months).toHaveLength(13);
    expect(definition.leap).toBeNull();
    await waitFor(() =>
      expect(onSaved).toHaveBeenCalledWith(expect.objectContaining({ id: "new-id" })),
    );
    // Saved: the draft is the baseline again.
    await waitFor(() => expect(onDirty).toHaveBeenLastCalledWith(false));
  });

  it("updates an existing calendar, sending its id, name and definition", async () => {
    renderWithQuery(<CalendarEditor calendar={ELVEN} onSaved={vi.fn()} onDirtyChange={vi.fn()} />);

    fireEvent.change(screen.getByRole("spinbutton", { name: "Epoch offset" }), {
      target: { value: "-1500" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));

    await waitFor(() => expect(commands.updateCalendar).toHaveBeenCalledTimes(1));
    const [id, name, definition] = commands.updateCalendar.mock.calls[0] as [
      string,
      string,
      CalendarDefinition,
    ];
    expect(id).toBe(ELVEN.id);
    expect(name).toBe("Elven Reckoning");
    expect(definition.epochOffset).toBe(-1500);
    expect(definition.leap).toEqual(ELVEN.definition.leap);
    expect(await screen.findByText("Saved.")).toBeInTheDocument();
  });

  it("shows save errors from the backend on the matching field", async () => {
    commands.createCalendar.mockResolvedValue({
      status: "error",
      error: { kind: "validation", message: "name: A calendar with that name already exists" },
    });
    renderWithQuery(<CalendarEditor calendar={null} onSaved={vi.fn()} onDirtyChange={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("Calendar name"), { target: { value: "Standard" } });
    fireEvent.click(screen.getByRole("button", { name: "Create calendar" }));

    expect(await screen.findByText("A calendar with that name already exists")).toBeInTheDocument();
    expect(screen.getByLabelText("Calendar name")).toHaveAttribute("aria-invalid", "true");
  });

  it("applies the Gregorian leap preset and sends the cycles", async () => {
    renderWithQuery(
      <CalendarEditor calendar={STANDARD} onSaved={vi.fn()} onDirtyChange={vi.fn()} />,
    );

    fireEvent.click(screen.getByLabelText("This calendar has leap years"));
    fireEvent.change(screen.getByLabelText("Leap preset"), { target: { value: "gregorian" } });
    expect(screen.getByLabelText("Leap cycle 3 every")).toHaveValue(400);

    await waitFor(() => expect(lastPreviewed().leap?.rules).toHaveLength(3));
    expect(lastPreviewed().leap).toMatchObject({ monthIndex: 0, extraDays: 1 });
    await waitFor(() =>
      expect(screen.getByRole("complementary", { name: "Preview" })).toHaveTextContent(
        "leap year: 361 days",
      ),
    );
  });

  it("reorders months with the arrow buttons", () => {
    renderWithQuery(
      <CalendarEditor calendar={STANDARD} onSaved={vi.fn()} onDirtyChange={vi.fn()} />,
    );
    expect(screen.getByLabelText("Month 1 name")).toHaveValue("Month 1");
    fireEvent.click(screen.getByRole("button", { name: "Move month 1 down" }));
    expect(screen.getByLabelText("Month 1 name")).toHaveValue("Month 2");
    expect(screen.getByLabelText("Month 2 name")).toHaveValue("Month 1");
    expect(screen.getByRole("button", { name: "Move month 1 up" })).toBeDisabled();
  });
});
