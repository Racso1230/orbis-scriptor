import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { STANDARD } from "@/features/calendars/test-calendars";
import type { FieldDef } from "@/lib/bindings";
import { FieldInput } from "./field-inputs";

const open = vi.hoisted(() => vi.fn());
vi.mock("./use-open-entity", () => ({ useOpenEntity: () => open }));
vi.mock("./queries", () => ({
  useEntities: () => ({
    data: [
      { id: "f1", type: "faction", name: "Tidewardens", updatedAt: "" },
      { id: "f2", type: "faction", name: "Ironbound", updatedAt: "" },
    ],
  }),
  useEntityTypes: () => ({
    data: [
      { key: "faction", label: "Faction", plural: "Factions", icon: "flag", color: "#dc2626" },
    ],
  }),
  useCreateEntity: () => ({ mutateAsync: vi.fn() }),
}));
vi.mock("@/features/calendars/queries", () => ({
  useCalendars: () => ({ data: [STANDARD] }),
  useDisplayCalendar: () => ({ data: STANDARD }),
}));
// The real picker is a cmdk dialog; a stub that picks "f2" keeps this test about the input.
vi.mock("@/features/maps/entity-picker-dialog", () => ({
  EntityPickerDialog: (props: {
    open: boolean;
    allowedTypes?: string[];
    onPickEntity: (id: string) => void;
  }) =>
    props.open ? (
      <button onClick={() => props.onPickEntity("f2")}>
        pick f2 (allowed: {(props.allowedTypes ?? []).join(",")})
      </button>
    ) : null,
}));

function field(partial: Partial<FieldDef> & Pick<FieldDef, "kind">): FieldDef {
  return { key: "f", label: "Field", options: [], custom: true, ...partial };
}

describe("FieldInput", () => {
  beforeEach(() => vi.clearAllMocks());

  it("multiSelect toggles options and reports them in option order", () => {
    const onChange = vi.fn();
    render(
      <FieldInput
        field={field({ kind: "multiSelect", label: "Domains", options: ["Sea", "War", "Death"] })}
        value={["Death"]}
        onChange={onChange}
      />,
    );
    expect(screen.getByLabelText("Death")).toBeChecked();
    fireEvent.click(screen.getByLabelText("Sea"));
    expect(onChange).toHaveBeenLastCalledWith(["Sea", "Death"]);
    fireEvent.click(screen.getByLabelText("Death"));
    expect(onChange).toHaveBeenLastCalledWith([]);
  });

  it("renders a stored value of the wrong shape as empty", () => {
    render(
      <FieldInput
        field={field({ kind: "select", label: "Domain", options: ["Sea"] })}
        value="Removed option"
        onChange={vi.fn()}
      />,
    );
    expect(screen.getByLabelText("Domain")).toHaveValue("");
  });

  it("boolean stores true and clears on uncheck", () => {
    const onChange = vi.fn();
    const f = field({ kind: "boolean", label: "Alive" });
    const { rerender } = render(<FieldInput field={f} value={undefined} onChange={onChange} />);
    fireEvent.click(screen.getByLabelText("Yes"));
    expect(onChange).toHaveBeenLastCalledWith(true);
    rerender(<FieldInput field={f} value={true} onChange={onChange} />);
    fireEvent.click(screen.getByLabelText("Yes"));
    expect(onChange).toHaveBeenLastCalledWith(undefined);
  });

  it("date shows a Set date button, then the editor with a working clear button", () => {
    const onChange = vi.fn();
    const f = field({ kind: "date", label: "Founded" });
    const { rerender } = render(<FieldInput field={f} value={undefined} onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: /Set date/ }));
    expect(onChange).toHaveBeenLastCalledWith(0);

    rerender(<FieldInput field={f} value={35} onChange={onChange} />);
    expect(screen.getByLabelText("Year")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Clear Founded" }));
    expect(onChange).toHaveBeenLastCalledWith(undefined);
  });

  it("entityRef multiple: shows chips, a missing chip, adds and removes", () => {
    const onChange = vi.fn();
    const f = field({
      kind: { entityRef: { allowedTypes: ["faction"], multiple: true } },
      label: "Worshippers",
    });
    render(<FieldInput field={f} value={["f1", "gone"]} onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: "Tidewardens" }));
    expect(open).toHaveBeenCalledWith("f1");
    expect(screen.getByText("missing entity")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Remove Tidewardens from Worshippers" }));
    expect(onChange).toHaveBeenLastCalledWith(["gone"]);
    fireEvent.click(screen.getByRole("button", { name: "Remove missing entity from Worshippers" }));
    expect(onChange).toHaveBeenLastCalledWith(["f1"]);

    fireEvent.click(screen.getByRole("button", { name: /Add/ }));
    // The picker is restricted to the allowed types.
    fireEvent.click(screen.getByRole("button", { name: /allowed: faction/ }));
    expect(onChange).toHaveBeenLastCalledWith(["f1", "gone", "f2"]);
  });

  it("entityRef single hides Add while set and clears to undefined", () => {
    const onChange = vi.fn();
    const f = field({
      kind: { entityRef: { allowedTypes: [], multiple: false } },
      label: "Patron",
    });
    const { rerender } = render(<FieldInput field={f} value="f1" onChange={onChange} />);
    expect(screen.queryByRole("button", { name: /Add/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Remove Tidewardens from Patron" }));
    expect(onChange).toHaveBeenLastCalledWith(undefined);

    rerender(<FieldInput field={f} value={undefined} onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: /Add/ }));
    fireEvent.click(screen.getByRole("button", { name: /pick f2/ }));
    expect(onChange).toHaveBeenLastCalledWith("f2");
  });

  it("shows a server error under the field it names", () => {
    render(
      <FieldInput
        field={field({ kind: "text", label: "Symbol" })}
        value="x"
        error="Symbol: too long"
        onChange={vi.fn()}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("too long");
  });

  it("number sends numbers and clears on empty", () => {
    const onChange = vi.fn();
    render(
      <FieldInput field={field({ kind: "number", label: "Age" })} value={3} onChange={onChange} />,
    );
    fireEvent.change(screen.getByLabelText("Age"), { target: { value: "7" } });
    expect(onChange).toHaveBeenLastCalledWith(7);
    fireEvent.change(screen.getByLabelText("Age"), { target: { value: "" } });
    expect(onChange).toHaveBeenLastCalledWith(undefined);
  });
});
