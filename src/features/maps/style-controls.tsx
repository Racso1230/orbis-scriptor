import { useEffect, useId, useRef, useState } from "react";
import { Label } from "@/components/ui/label";

/**
 * A draft copy of `value` that follows outside changes (undo/redo, selecting
 * another object). Resets during render, which React allows for derived state.
 */
function useDraft<T>(value: T): [T, (next: T) => void] {
  const [state, setState] = useState({ base: value, draft: value });
  let current = state;
  if (state.base !== value) {
    current = { base: value, draft: value };
    setState(current);
  }
  return [current.draft, (draft) => setState({ base: value, draft })];
}

/**
 * Small form controls for the properties panel. Each keeps a local "draft"
 * while you drag or pick, and calls `onCommit` once when you let go, so a
 * single slider drag becomes ONE undo step instead of dozens.
 */

interface ColorProps {
  label: string;
  value: string;
  disabled?: boolean;
  onCommit: (value: string) => void;
}

export function ColorControl({ label, value, disabled, onCommit }: ColorProps) {
  const id = useId();
  const ref = useRef<HTMLInputElement>(null);
  const [draft, setDraft] = useDraft(value);
  const latest = useRef({ onCommit, value });
  useEffect(() => {
    latest.current = { onCommit, value };
  });

  // The browser's native "change" event fires when the picker closes; React's
  // onChange fires on every mouse move inside it, so listen natively to commit.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    function onChange() {
      if (el && el.value !== latest.current.value) latest.current.onCommit(el.value);
    }
    el.addEventListener("change", onChange);
    return () => el.removeEventListener("change", onChange);
  }, []);

  return (
    <div className="flex items-center justify-between gap-2">
      <Label htmlFor={id} className="text-xs">
        {label}
      </Label>
      <input
        ref={ref}
        id={id}
        type="color"
        disabled={disabled}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        className="h-6 w-10 cursor-pointer rounded border bg-transparent p-0 disabled:cursor-not-allowed"
      />
    </div>
  );
}

interface RangeProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  disabled?: boolean;
  /** Text shown next to the label, e.g. "25%". */
  format?: (value: number) => string;
  onCommit: (value: number) => void;
}

export function RangeControl({
  label,
  value,
  min,
  max,
  step,
  disabled,
  format,
  onCommit,
}: RangeProps) {
  const id = useId();
  const [draft, setDraft] = useDraft(value);

  function commit() {
    if (draft !== value) onCommit(draft);
  }

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between">
        <Label htmlFor={id} className="text-xs">
          {label}
        </Label>
        <span className="text-xs text-muted-foreground">
          {format ? format(draft) : String(draft)}
        </span>
      </div>
      <input
        id={id}
        type="range"
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        value={draft}
        onChange={(e) => setDraft(Number(e.target.value))}
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
        className="w-full accent-primary"
      />
    </div>
  );
}

interface SelectProps<T extends string> {
  label: string;
  value: T;
  options: readonly { value: T; label: string }[];
  disabled?: boolean;
  onCommit: (value: T) => void;
}

/** A native select: simple, accessible and committed on change. */
export function SelectControl<T extends string>({
  label,
  value,
  options,
  disabled,
  onCommit,
}: SelectProps<T>) {
  const id = useId();
  return (
    <div className="flex items-center justify-between gap-2">
      <Label htmlFor={id} className="text-xs">
        {label}
      </Label>
      <select
        id={id}
        value={value}
        disabled={disabled}
        onChange={(e) => onCommit(e.target.value as T)}
        className="h-7 rounded-md border bg-background px-1.5 text-xs"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </div>
  );
}
