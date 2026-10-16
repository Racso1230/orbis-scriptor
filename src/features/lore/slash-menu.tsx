import { forwardRef, useEffect, useImperativeHandle, useState } from "react";
import { cn } from "@/lib/utils";
/** What a menu row needs; slash items and mention items both satisfy it. */
export interface MenuEntry {
  title: string;
  description: string;
}

export interface SlashMenuProps {
  items: MenuEntry[];
  // Method syntax (not an arrow type) so callers may pass a handler typed for
  // their own narrower item type.
  command(item: MenuEntry): void;
  /** Where to show the menu (from the suggestion plugin's clientRect). */
  rect: DOMRect | null;
}

export interface SlashMenuHandle {
  onKeyDown: (event: KeyboardEvent) => boolean;
}

/** The floating list shown after typing `/`. Keyboard handling is driven by TipTap. */
export const SlashMenu = forwardRef<SlashMenuHandle, SlashMenuProps>(function SlashMenu(
  { items, command, rect },
  ref,
) {
  const [selected, setSelected] = useState(0);

  useEffect(() => setSelected(0), [items]);

  useImperativeHandle(ref, () => ({
    onKeyDown(event) {
      if (event.key === "ArrowUp") {
        setSelected((s) => (s + items.length - 1) % items.length);
        return true;
      }
      if (event.key === "ArrowDown") {
        setSelected((s) => (s + 1) % items.length);
        return true;
      }
      if (event.key === "Enter") {
        const item = items[selected];
        if (item) command(item);
        return true;
      }
      return false;
    },
  }));

  if (!rect) return null;

  return (
    <div
      role="listbox"
      style={{ position: "fixed", left: rect.left, top: rect.bottom + 4 }}
      className="z-50 w-64 overflow-hidden rounded-md border bg-popover p-1 text-popover-foreground shadow-md"
    >
      {items.length === 0 && (
        <p className="px-2 py-1.5 text-sm text-muted-foreground">No results</p>
      )}
      {items.map((item, index) => (
        <button
          key={item.title}
          type="button"
          role="option"
          aria-selected={index === selected}
          onMouseEnter={() => setSelected(index)}
          onClick={() => command(item)}
          className={cn(
            "flex w-full flex-col items-start rounded-sm px-2 py-1.5 text-left text-sm",
            index === selected && "bg-accent text-accent-foreground",
          )}
        >
          <span>{item.title}</span>
          <span className="text-xs text-muted-foreground">{item.description}</span>
        </button>
      ))}
    </div>
  );
});
