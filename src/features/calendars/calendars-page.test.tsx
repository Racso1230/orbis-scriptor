import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithQuery } from "@/test/render";
import { CalendarsPage } from "./calendars-page";
import { ELVEN, STANDARD } from "./test-calendars";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const commands = vi.hoisted(() => ({
  listCalendars: vi.fn(),
  previewCalendar: vi.fn(),
  setDefaultCalendar: vi.fn(),
  duplicateCalendar: vi.fn(),
  deleteCalendar: vi.fn(),
  createCalendar: vi.fn(),
  updateCalendar: vi.fn(),
}));
vi.mock("@/lib/bindings", () => ({ commands }));
const useBlocker = vi.hoisted(() => vi.fn());
vi.mock("@tanstack/react-router", () => ({ useBlocker }));

describe("CalendarsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.listCalendars.mockResolvedValue(ok([STANDARD, ELVEN]));
    commands.previewCalendar.mockImplementation((_d: unknown, days: number[]) =>
      Promise.resolve({ issues: [], labels: days.map(() => "x") }),
    );
    commands.setDefaultCalendar.mockResolvedValue(ok({ ...ELVEN, isDefault: true }));
    commands.deleteCalendar.mockResolvedValue(ok(null));
    commands.duplicateCalendar.mockImplementation((_id: string, newName: string) =>
      Promise.resolve(ok({ ...ELVEN, id: "copy-id", name: newName, isDefault: false })),
    );
  });
  afterEach(() => vi.restoreAllMocks());

  it("lists the calendars with a default badge and opens the default in the editor", async () => {
    renderWithQuery(<CalendarsPage />);
    const list = await screen.findByRole("region", { name: "Calendars" });
    expect(within(list).getByText("Standard")).toBeInTheDocument();
    expect(within(list).getByText("Elven Reckoning")).toBeInTheDocument();
    expect(within(list).getAllByText("Default")).toHaveLength(1);
    expect(await screen.findByLabelText("Calendar name")).toHaveValue("Standard");
  });

  it("disables deleting the default calendar", async () => {
    renderWithQuery(<CalendarsPage />);
    expect(await screen.findByRole("button", { name: "Delete Standard" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Delete Elven Reckoning" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Make Standard the default" })).toBeDisabled();
  });

  it("sets another calendar as the default", async () => {
    renderWithQuery(<CalendarsPage />);
    fireEvent.click(
      await screen.findByRole("button", { name: "Make Elven Reckoning the default" }),
    );
    await waitFor(() => expect(commands.setDefaultCalendar).toHaveBeenCalledWith(ELVEN.id));
  });

  it("deletes a calendar only after confirming", async () => {
    renderWithQuery(<CalendarsPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Delete Elven Reckoning" }));
    expect(commands.deleteCalendar).not.toHaveBeenCalled();
    expect(await screen.findByText(/Events keep their dates/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^Delete$/ }));
    await waitFor(() => expect(commands.deleteCalendar).toHaveBeenCalledWith(ELVEN.id));
  });

  it("duplicates a calendar under a new unique name and opens the copy", async () => {
    renderWithQuery(<CalendarsPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Duplicate Elven Reckoning" }));
    await waitFor(() =>
      expect(commands.duplicateCalendar).toHaveBeenCalledWith(ELVEN.id, "Elven Reckoning (copy)"),
    );
  });

  it("asks before switching calendars with unsaved edits", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    renderWithQuery(<CalendarsPage />);
    fireEvent.change(await screen.findByLabelText("Calendar name"), {
      target: { value: "Renamed" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Edit Elven Reckoning" }));
    expect(confirm).toHaveBeenCalledTimes(1);
    // Declined: still editing the first calendar with the typed name.
    expect(screen.getByLabelText("Calendar name")).toHaveValue("Renamed");

    confirm.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "Edit Elven Reckoning" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Calendar name")).toHaveValue("Elven Reckoning"),
    );
  });

  it("blocks leaving the page only while there are unsaved edits", async () => {
    renderWithQuery(<CalendarsPage />);
    const nameInput = await screen.findByLabelText("Calendar name");
    expect(useBlocker.mock.calls.at(-1)?.[0]).toMatchObject({ disabled: true });

    fireEvent.change(nameInput, { target: { value: "Changed" } });
    await waitFor(() =>
      expect(useBlocker.mock.calls.at(-1)?.[0]).toMatchObject({ disabled: false }),
    );

    // The blocker's callback asks with a confirm: blocking when the user declines.
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const options = useBlocker.mock.calls.at(-1)?.[0] as { shouldBlockFn: () => boolean };
    expect(options.shouldBlockFn()).toBe(true);
    confirm.mockReturnValue(true);
    expect(options.shouldBlockFn()).toBe(false);
  });
});
