import { useId, useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { CalendarDefinition } from "@/lib/bindings";
import { fromDate, toDate } from "./calendar";

interface Props {
  def: CalendarDefinition;
  /** Day number from the world epoch. */
  day: number;
  label: string;
  onChange: (day: number) => void;
}

/**
 * Edits one day number as era / year / month / day. It keeps its own text
 * state while typing and only reports a day number when the parts are valid.
 */
export function DateEditor({ def, day, label, onChange }: Props) {
  const id = useId();
  const initial = toDate(def, day);
  const [eraIndex, setEraIndex] = useState(initial.eraIndex);
  const [year, setYear] = useState(String(initial.year));
  const [month, setMonth] = useState(initial.month);
  const [dayText, setDayText] = useState(String(initial.day));

  const monthLength = def.months[month - 1]?.days ?? 1;

  function commit(next: { eraIndex: number; year: string; month: number; dayText: string }) {
    const y = Number(next.year);
    const d = Number(next.dayText);
    const length = def.months[next.month - 1]?.days ?? 1;
    if (next.year.trim() === "" || !Number.isInteger(y)) return;
    if (next.dayText.trim() === "" || !Number.isInteger(d) || d < 1 || d > length) return;
    onChange(fromDate(def, next.eraIndex, y, next.month, d));
  }

  const current = { eraIndex, year, month, dayText };
  const dayInvalid =
    dayText.trim() === "" ||
    !Number.isInteger(Number(dayText)) ||
    Number(dayText) < 1 ||
    Number(dayText) > monthLength;

  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="mb-2 text-sm font-medium">{label}</legend>
      <div className="flex flex-wrap items-end gap-3">
        <div className="flex w-20 flex-col gap-1">
          <Label htmlFor={`${id}-day`} className="text-xs text-muted-foreground">
            Day
          </Label>
          <Input
            id={`${id}-day`}
            type="number"
            min={1}
            max={monthLength}
            value={dayText}
            aria-invalid={dayInvalid}
            onChange={(e) => {
              setDayText(e.target.value);
              commit({ ...current, dayText: e.target.value });
            }}
          />
        </div>
        <div className="flex w-44 flex-col gap-1">
          <Label htmlFor={`${id}-month`} className="text-xs text-muted-foreground">
            Month
          </Label>
          <Select
            value={String(month)}
            onValueChange={(v) => {
              const m = Number(v);
              setMonth(m);
              commit({ ...current, month: m });
            }}
          >
            <SelectTrigger id={`${id}-month`} className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {def.months.map((m, i) => (
                <SelectItem key={i} value={String(i + 1)}>
                  {m.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex w-28 flex-col gap-1">
          <Label htmlFor={`${id}-year`} className="text-xs text-muted-foreground">
            Year
          </Label>
          <Input
            id={`${id}-year`}
            type="number"
            value={year}
            onChange={(e) => {
              setYear(e.target.value);
              commit({ ...current, year: e.target.value });
            }}
          />
        </div>
        {def.eras.length > 1 && (
          <div className="flex w-40 flex-col gap-1">
            <Label htmlFor={`${id}-era`} className="text-xs text-muted-foreground">
              Era
            </Label>
            <Select
              value={String(eraIndex)}
              onValueChange={(v) => {
                const e = Number(v);
                setEraIndex(e);
                commit({ ...current, eraIndex: e });
              }}
            >
              <SelectTrigger id={`${id}-era`} className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {def.eras.map((era, i) => (
                  <SelectItem key={i} value={String(i)}>
                    {era.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        )}
        {def.eras.length === 1 && (
          <span className="pb-2 text-sm text-muted-foreground">
            {def.eras[0]?.abbreviation ?? def.eras[0]?.name}
          </span>
        )}
      </div>
      {dayInvalid && (
        <p className="text-xs text-destructive">Day must be between 1 and {monthLength}.</p>
      )}
    </fieldset>
  );
}
