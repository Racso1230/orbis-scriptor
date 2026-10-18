import type { ComponentType } from "react";
import { Hexagon, MapPin, MousePointer2, Pencil, Spline, type LucideIcon } from "lucide-react";
import { AreaTool, PathTool } from "./polyline-tool";
import { FreehandTool } from "./freehand-tool";
import type { MapTool } from "./map-tool-store";
import type { ToolOverlayProps } from "./tool-types";

export interface ToolDefinition {
  id: MapTool;
  /** Button label and tooltip, without the shortcut. */
  label: string;
  /** Single key (no modifiers) that selects the tool; case-insensitive. */
  shortcut: string;
  icon: LucideIcon;
  /**
   * Drawing tools render their live preview + pointer handling here (see
   * `tool-types.ts`). Tools without one (Select, Pin) are handled by the
   * canvas itself. A tool with an overlay turns object hit-testing off while
   * active, so its clicks reach the stage.
   */
  Overlay?: ComponentType<ToolOverlayProps>;
  /** Writes to the map, so it needs an editable (visible, unlocked) active layer. */
  needsEditableLayer: boolean;
  /** CSS cursor over the canvas. */
  cursor?: string;
  /** Left-drag pans the map. False for tools where left-drag draws. */
  leftDragPans: boolean;
}

/**
 * The tool registry, in toolbar order. To add a tool (Part 3: Label, Stamp):
 * extend `MapTool` in `map-tool-store.ts`, add an entry here (and an
 * `Overlay` component that follows `tool-types.ts`), and it appears in the
 * toolbar and gets its keyboard shortcut automatically.
 */
export const TOOLS: readonly ToolDefinition[] = [
  {
    id: "select",
    label: "Select",
    shortcut: "V",
    icon: MousePointer2,
    needsEditableLayer: false,
    leftDragPans: true,
  },
  {
    id: "pin",
    label: "Pin",
    shortcut: "P",
    icon: MapPin,
    needsEditableLayer: true,
    cursor: "crosshair",
    leftDragPans: true,
  },
  {
    id: "path",
    label: "Path",
    shortcut: "L",
    icon: Spline,
    Overlay: PathTool,
    needsEditableLayer: true,
    cursor: "crosshair",
    leftDragPans: true,
  },
  {
    id: "freehand",
    label: "Freehand",
    shortcut: "F",
    icon: Pencil,
    Overlay: FreehandTool,
    needsEditableLayer: true,
    cursor: "crosshair",
    leftDragPans: false,
  },
  {
    id: "area",
    label: "Area",
    shortcut: "A",
    icon: Hexagon,
    Overlay: AreaTool,
    needsEditableLayer: true,
    cursor: "crosshair",
    leftDragPans: true,
  },
];

export function toolById(id: MapTool): ToolDefinition {
  const found = TOOLS.find((t) => t.id === id);
  if (!found) throw new Error(`Unknown map tool: ${id}`);
  return found;
}

/** The tool whose shortcut key matches, if any. */
export function toolForKey(key: string): ToolDefinition | undefined {
  const k = key.toLowerCase();
  return TOOLS.find((t) => t.shortcut.toLowerCase() === k);
}
