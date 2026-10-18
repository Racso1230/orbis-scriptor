import type { ReactNode } from "react";
import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import { NativeSelect } from "@/components/native-select";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  LEAP_PRESETS,
  addMonth,
  applyLeapPreset,
  detectLeapPreset,
  moveItem,
  moveMonth,
  newKey,
  removeMonth,
  type CalendarDraft,
  type LeapPreset,
} from "./draft";

/** Field path -> message, from `issuesByField`. */
export type FieldErrors = Record<string, string>;

interface SectionProps {
  draft: CalendarDraft;
  errors: FieldErrors;
  onChange: (next: CalendarDraft) => void;
}

function FieldError({ id, message }: { id: string; message: string | undefined }) {
  if (!message) return null;
  return (
    <p id={id} className="text-xs text-destructive">
      {message}
    </p>
  );
}

function Section({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="flex flex-col gap-3 rounded-lg border p-4">
      <div>
        <h3 className="text-sm font-semibold">{title}</h3>
        {hint && <p className="mt-1 text-xs text-muted-foreground">{hint}</p>}
      </div>
      {children}
    </section>
  );
}

/** Up / down / remove buttons for one row of a list. */
function RowButtons({
  noun,
  position,
  count,
  onMove,
  onRemove,
  removeDisabled = false,
}: {
  noun: string;
  position: number;
  count: number;
  onMove: (delta: number) => void;
  onRemove: () => void;
  removeDisabled?: boolean;
}) {
  return (
    <div className="flex shrink-0 items-center">
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={`Move ${noun} ${position + 1} up`}
        disabled={position === 0}
        onClick={() => onMove(-1)}
      >
        <ArrowUp />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={`Move ${noun} ${position + 1} down`}
        disabled={position === count - 1}
        onClick={() => onMove(1)}
      >
        <ArrowDown />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={`Remove ${noun} ${position + 1}`}
        disabled={removeDisabled}
        onClick={onRemove}
      >
        <Trash2 />
      </Button>
    </div>
  );
}

export function MonthsSection({ draft, errors, onChange }: SectionProps) {
  const { months } = draft;
  return (
    <Section title="Months" hint="A year is the sum of its months. Leap days are added below.">
      <FieldError id="err-months" message={errors["months"]} />
      <ul className="flex flex-col gap-2">
        {months.map((m, i) => (
          <li key={m.key} className="flex flex-col gap-1">
            <div className="flex items-center gap-2">
              <span className="w-6 text-right text-xs text-muted-foreground">{i + 1}</span>
              <Input
                aria-label={`Month ${i + 1} name`}
                aria-invalid={errors[`months[${i}].name`] ? true : undefined}
                value={m.name}
                onChange={(e) =>
                  onChange({
                    ...draft,
                    months: months.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)),
                  })
                }
              />
              <Input
                aria-label={`Month ${i + 1} days`}
                aria-invalid={errors[`months[${i}].days`] ? true : undefined}
                className="w-24"
                type="number"
                value={m.days}
                onChange={(e) =>
                  onChange({
                    ...draft,
                    months: months.map((x, j) => (j === i ? { ...x, days: e.target.value } : x)),
                  })
                }
              />
              <span className="text-xs text-muted-foreground">days</span>
              <RowButtons
                noun="month"
                position={i}
                count={months.length}
                onMove={(delta) => onChange(moveMonth(draft, i, delta))}
                onRemove={() => onChange(removeMonth(draft, i))}
                removeDisabled={months.length <= 1}
              />
            </div>
            <FieldError id={`err-month-name-${i}`} message={errors[`months[${i}].name`]} />
            <FieldError id={`err-month-days-${i}`} message={errors[`months[${i}].days`]} />
          </li>
        ))}
      </ul>
      <div>
        <Button variant="outline" size="sm" onClick={() => onChange(addMonth(draft))}>
          <Plus /> Add month
        </Button>
      </div>
    </Section>
  );
}

export function WeekdaysSection({ draft, errors, onChange }: SectionProps) {
  const { weekdays } = draft;
  return (
    <Section
      title="Weekdays"
      hint="Optional. Weekdays repeat in order through the whole timeline; with none, dates show no weekday."
    >
      <FieldError id="err-weekdays" message={errors["weekdays"]} />
      <ul className="flex flex-col gap-2">
        {weekdays.map((w, i) => (
          <li key={w.key} className="flex flex-col gap-1">
            <div className="flex items-center gap-2">
              <span className="w-6 text-right text-xs text-muted-foreground">{i + 1}</span>
              <Input
                aria-label={`Weekday ${i + 1} name`}
                aria-invalid={errors[`weekdays[${i}]`] ? true : undefined}
                value={w.name}
                onChange={(e) =>
                  onChange({
                    ...draft,
                    weekdays: weekdays.map((x, j) =>
                      j === i ? { ...x, name: e.target.value } : x,
                    ),
                  })
                }
              />
              <RowButtons
                noun="weekday"
                position={i}
                count={weekdays.length}
                onMove={(delta) => onChange({ ...draft, weekdays: moveItem(weekdays, i, delta) })}
                onRemove={() =>
                  onChange({ ...draft, weekdays: weekdays.filter((_, j) => j !== i) })
                }
              />
            </div>
            <FieldError id={`err-weekday-${i}`} message={errors[`weekdays[${i}]`]} />
          </li>
        ))}
      </ul>
      <div>
        <Button
          variant="outline"
          size="sm"
          onClick={() =>
            onChange({
              ...draft,
              weekdays: [...weekdays, { key: newKey(), name: `Day ${weekdays.length + 1}` }],
            })
          }
        >
          <Plus /> Add weekday
        </Button>
      </div>
    </Section>
  );
}

export function ErasSection({
  draft,
  errors,
  onChange,
  eraDates,
}: SectionProps & {
  /** The live-formatted start date of each era (from the preview), when known. */
  eraDates: readonly (string | undefined)[];
}) {
  const { eras } = draft;
  return (
    <Section
      title="Eras"
      hint="An era starts on an absolute day number, shared by every calendar, and its first year is year 1. Keep them in order, earliest first."
    >
      <FieldError id="err-eras" message={errors["eras"]} />
      <ul className="flex flex-col gap-3">
        {eras.map((era, i) => (
          <li key={era.key} className="flex flex-col gap-1">
            <div className="flex flex-wrap items-center gap-2">
              <Input
                aria-label={`Era ${i + 1} name`}
                aria-invalid={errors[`eras[${i}].name`] ? true : undefined}
                className="w-48"
                value={era.name}
                onChange={(e) =>
                  onChange({
                    ...draft,
                    eras: eras.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)),
                  })
                }
              />
              <Input
                aria-label={`Era ${i + 1} abbreviation`}
                aria-invalid={errors[`eras[${i}].abbreviation`] ? true : undefined}
                className="w-24"
                placeholder="Abbr."
                value={era.abbreviation}
                onChange={(e) =>
                  onChange({
                    ...draft,
                    eras: eras.map((x, j) =>
                      j === i ? { ...x, abbreviation: e.target.value } : x,
                    ),
                  })
                }
              />
              <span className="text-xs text-muted-foreground">starts on day</span>
              <Input
                aria-label={`Era ${i + 1} start day`}
                aria-invalid={errors[`eras[${i}].startDay`] ? true : undefined}
                className="w-36"
                type="number"
                value={era.startDay}
                onChange={(e) =>
                  onChange({
                    ...draft,
                    eras: eras.map((x, j) => (j === i ? { ...x, startDay: e.target.value } : x)),
                  })
                }
              />
              <RowButtons
                noun="era"
                position={i}
                count={eras.length}
                onMove={(delta) => onChange({ ...draft, eras: moveItem(eras, i, delta) })}
                onRemove={() => onChange({ ...draft, eras: eras.filter((_, j) => j !== i) })}
                removeDisabled={eras.length <= 1}
              />
            </div>
            {eraDates[i] && (
              <p className="text-xs text-muted-foreground">Starts on {eraDates[i]}</p>
            )}
            <FieldError id={`err-era-name-${i}`} message={errors[`eras[${i}].name`]} />
            <FieldError id={`err-era-abbr-${i}`} message={errors[`eras[${i}].abbreviation`]} />
            <FieldError id={`err-era-start-${i}`} message={errors[`eras[${i}].startDay`]} />
          </li>
        ))}
      </ul>
      <div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => {
            const last = eras[eras.length - 1];
            const lastStart = Number(last?.startDay);
            onChange({
              ...draft,
              eras: [
                ...eras,
                {
                  key: newKey(),
                  name: `Era ${eras.length + 1}`,
                  abbreviation: "",
                  startDay: String(Number.isFinite(lastStart) ? lastStart + 1000 : 0),
                },
              ],
            });
          }}
        >
          <Plus /> Add era
        </Button>
      </div>
    </Section>
  );
}

export function EpochSection({
  draft,
  errors,
  onChange,
  dayZeroLabel,
}: SectionProps & {
  /** How the preview formats absolute day 0, when known. */
  dayZeroLabel: string | undefined;
}) {
  return (
    <Section
      title="Epoch offset"
      hint="Which absolute day is year 1, day 1 of this calendar. Changing it shifts how every date is shown in this calendar; events never move."
    >
      <div className="flex items-center gap-2">
        <Input
          aria-label="Epoch offset"
          aria-invalid={errors["epochOffset"] ? true : undefined}
          className="w-44"
          type="number"
          value={draft.epochOffset}
          onChange={(e) => onChange({ ...draft, epochOffset: e.target.value })}
        />
        <span className="text-xs text-muted-foreground">absolute day</span>
      </div>
      <FieldError id="err-epoch" message={errors["epochOffset"]} />
      {dayZeroLabel && (
        <p className="text-xs text-muted-foreground">Absolute day 0 reads as {dayZeroLabel}.</p>
      )}
    </Section>
  );
}

export function LeapSection({ draft, errors, onChange }: SectionProps) {
  const { leap, months } = draft;
  const preset = detectLeapPreset(leap);

  function changePreset(id: LeapPreset["id"] | "custom") {
    if (id !== "custom") onChange(applyLeapPreset(draft, id));
  }

  return (
    <Section
      title="Leap years"
      hint="One month gains extra days in leap years. A year is leap when the largest cycle that divides its number says so."
    >
      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={leap.enabled}
          onChange={(e) =>
            onChange(
              !e.target.checked
                ? { ...draft, leap: { ...leap, enabled: false } }
                : leap.rules.length > 0
                  ? { ...draft, leap: { ...leap, enabled: true } }
                  : applyLeapPreset(draft, "every4"),
            )
          }
        />
        This calendar has leap years
      </label>

      {leap.enabled && (
        <>
          <div className="flex flex-wrap items-end gap-3">
            <label className="flex flex-col gap-1 text-xs text-muted-foreground">
              Preset
              <NativeSelect
                aria-label="Leap preset"
                value={preset ?? "custom"}
                onChange={(e) => changePreset(e.target.value as LeapPreset["id"] | "custom")}
              >
                {LEAP_PRESETS.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.label}
                  </option>
                ))}
                {preset === null && <option value="custom">Custom</option>}
              </NativeSelect>
            </label>
            <label className="flex flex-col gap-1 text-xs text-muted-foreground">
              Month that gains days
              <NativeSelect
                aria-label="Leap month"
                aria-invalid={errors["leap.monthIndex"] ? true : undefined}
                value={String(leap.monthIndex)}
                onChange={(e) =>
                  onChange({ ...draft, leap: { ...leap, monthIndex: Number(e.target.value) } })
                }
              >
                {months.map((m, i) => (
                  <option key={m.key} value={String(i)}>
                    {m.name || `Month ${i + 1}`}
                  </option>
                ))}
              </NativeSelect>
            </label>
            <label className="flex flex-col gap-1 text-xs text-muted-foreground">
              Extra days
              <Input
                aria-label="Extra days"
                aria-invalid={errors["leap.extraDays"] ? true : undefined}
                className="w-24"
                type="number"
                value={leap.extraDays}
                onChange={(e) =>
                  onChange({ ...draft, leap: { ...leap, extraDays: e.target.value } })
                }
              />
            </label>
          </div>
          <FieldError id="err-leap-month" message={errors["leap.monthIndex"]} />
          <FieldError id="err-leap-extra" message={errors["leap.extraDays"]} />
          <FieldError id="err-leap-rules" message={errors["leap.rules"]} />

          <ul className="flex flex-col gap-2">
            {leap.rules.map((rule, i) => (
              <li key={rule.key} className="flex flex-col gap-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-sm">Every</span>
                  <Input
                    aria-label={`Leap cycle ${i + 1} every`}
                    aria-invalid={errors[`leap.rules[${i}].every`] ? true : undefined}
                    className="w-24"
                    type="number"
                    value={rule.every}
                    onChange={(e) =>
                      onChange({
                        ...draft,
                        leap: {
                          ...leap,
                          rules: leap.rules.map((r, j) =>
                            j === i ? { ...r, every: e.target.value } : r,
                          ),
                        },
                      })
                    }
                  />
                  <span className="text-sm">years</span>
                  <NativeSelect
                    aria-label={`Leap cycle ${i + 1} effect`}
                    value={rule.add ? "add" : "remove"}
                    onChange={(e) =>
                      onChange({
                        ...draft,
                        leap: {
                          ...leap,
                          rules: leap.rules.map((r, j) =>
                            j === i ? { ...r, add: e.target.value === "add" } : r,
                          ),
                        },
                      })
                    }
                  >
                    <option value="add">is a leap year</option>
                    <option value="remove">is not a leap year</option>
                  </NativeSelect>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label={`Remove leap cycle ${i + 1}`}
                    onClick={() =>
                      onChange({
                        ...draft,
                        leap: { ...leap, rules: leap.rules.filter((_, j) => j !== i) },
                      })
                    }
                  >
                    <Trash2 />
                  </Button>
                </div>
                <FieldError id={`err-leap-every-${i}`} message={errors[`leap.rules[${i}].every`]} />
              </li>
            ))}
          </ul>
          <div>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                const last = Number(leap.rules[leap.rules.length - 1]?.every);
                const every = Number.isFinite(last) && last > 0 ? last * 4 : 4;
                onChange({
                  ...draft,
                  leap: {
                    ...leap,
                    rules: [
                      ...leap.rules,
                      { key: newKey(), every: String(every), add: leap.rules.length % 2 === 1 },
                    ],
                  },
                });
              }}
            >
              <Plus /> Add cycle
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">
            Cycles run from the smallest to the largest, and each must divide the next (for example
            4, 100, 400). Year numbers count from the calendar&apos;s year 1.
          </p>
        </>
      )}
    </Section>
  );
}
