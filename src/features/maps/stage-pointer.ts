import type Konva from "konva";
import type { Point } from "./geometry";
import { screenToMap, type Transform } from "./viewport";

/** The stage's current pan/zoom, read straight from Konva (always up to date). */
export function stageTransform(stage: Konva.Stage): Transform {
  return { scale: stage.scaleX(), x: stage.x(), y: stage.y() };
}

/** Pointer position in map pixels, or `null` when the pointer is not over the stage. */
export function pointerInMap(stage: Konva.Stage): Point | null {
  const pointer = stage.getPointerPosition();
  if (!pointer) return null;
  return screenToMap(stageTransform(stage), pointer);
}

/** Keeps a point inside the map rectangle. */
export function clampToMap(point: Point, width: number, height: number): Point {
  return {
    x: Math.min(width, Math.max(0, point.x)),
    y: Math.min(height, Math.max(0, point.y)),
  };
}

/** True when the keyboard event came from somewhere the user is typing. */
export function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}
