import type Konva from "konva";

/**
 * Konva node name for things that keep a constant on-screen size at any zoom
 * (pins, vertex handles, drawing dots). The stage scales everything by the
 * zoom, so these nodes are given a scale of 1/zoom to cancel it out.
 */
export const SCREEN_FIXED = "screen-fixed";

/** Sets the counter-scale on every screen-fixed node below `root`. */
export function applyScreenFixed(root: Konva.Container, stageScale: number): void {
  const inverse = 1 / stageScale;
  for (const node of root.find(`.${SCREEN_FIXED}`)) {
    node.scale({ x: inverse, y: inverse });
  }
}
