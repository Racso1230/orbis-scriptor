import type { ReactNode } from "react";
import type Konva from "konva";
import { Group } from "react-konva";
import type { MapObject } from "@/lib/bindings";
import type { Point } from "./geometry";
import type { ObjectHandlers } from "./render-types";
import { SCREEN_FIXED } from "./screen-fixed";

interface Props {
  object: MapObject;
  /**
   * Position of the shell's origin. Pins, labels and stamps pass their x/y;
   * paths and areas pass nothing, so their points stay in map coordinates.
   */
  anchor?: Point;
  /** Counter-scale by 1/zoom (the canvas applies it); used by pins. */
  screenFixed?: boolean;
  /** Rotation in degrees (clockwise), applied to the shell. */
  rotation?: number;
  selectable: boolean;
  draggable: boolean;
  handlers: ObjectHandlers;
  children: ReactNode;
}

/**
 * The common wrapper every renderer puts around its Konva shapes: it owns
 * click-to-select, double-click-to-open, hover and drag-to-move, so a new
 * kind (labels, stamps...) only has to draw its shapes.
 */
export function ObjectShell({
  object,
  anchor,
  screenFixed = false,
  rotation = 0,
  selectable,
  draggable,
  handlers,
  children,
}: Props) {
  const ax = anchor?.x ?? 0;
  const ay = anchor?.y ?? 0;
  return (
    <Group
      id={object.id}
      name={screenFixed ? SCREEN_FIXED : undefined}
      x={ax}
      y={ay}
      rotation={rotation}
      draggable={draggable}
      onClick={(e) => {
        e.cancelBubble = true;
        if (selectable) handlers.onSelect(object.id);
      }}
      onTap={(e) => {
        e.cancelBubble = true;
        if (selectable) handlers.onSelect(object.id);
      }}
      onDblClick={(e) => {
        e.cancelBubble = true;
        handlers.onOpen(object);
      }}
      onDblTap={(e) => {
        e.cancelBubble = true;
        handlers.onOpen(object);
      }}
      onMouseEnter={() => handlers.onHover(object.id)}
      onMouseLeave={() => handlers.onHover(null)}
      onDragStart={() => {
        handlers.onHover(null);
        handlers.onSelect(object.id);
      }}
      onDragEnd={(e: Konva.KonvaEventObject<DragEvent>) => {
        e.cancelBubble = true; // do not let the stage think it was panned
        const node = e.target;
        const dx = node.x() - ax;
        const dy = node.y() - ay;
        // Anchored shells follow their prop; origin shells must be put back,
        // because their points (not the group) hold the new position.
        if (!anchor) node.position({ x: 0, y: 0 });
        if (dx !== 0 || dy !== 0) handlers.onMove(object.id, dx, dy);
      }}
    >
      {children}
    </Group>
  );
}
