export const MIN_TREE_WIDTH = 140;
export const DEFAULT_TREE_WIDTH = 260;
export const MIN_TERMINAL_BOTTOM = 120;
export const MIN_TERMINAL_RIGHT = 200;
export const DEFAULT_TERMINAL_BOTTOM = 280;
export const DEFAULT_TERMINAL_RIGHT = 380;

/** No panel may eat more than this share of the window. */
export const MAX_PANEL_RATIO = 0.8;

export const TREE_WIDTH_STEP = 16;

export function panelMaximum(minimum: number, viewport: number): number {
  if (!Number.isFinite(viewport) || viewport <= 0) return minimum;
  return Math.max(minimum, Math.floor(viewport * MAX_PANEL_RATIO));
}

/** Rounds and keeps a panel size inside [minimum, 80% of the viewport]. */
export function clampPanelSize(pixels: number, minimum: number, viewport: number): number {
  if (!Number.isFinite(pixels)) return minimum;

  const maximum = panelMaximum(minimum, viewport);
  return Math.min(maximum, Math.max(minimum, Math.round(pixels)));
}

export interface DragOrigin {
  /** Pointer coordinate when the drag started, on the axis being resized. */
  start: number;
  startSize: number;
  /**
   * Which way the panel grows. The tree sits before its handle, so moving the
   * pointer forward makes it wider ("forward"). The terminal sits after its
   * handle, so it grows when the pointer moves back ("backward").
   */
  grow: "forward" | "backward";
}

/** Raw size for a pointer position, before clamping. */
export function dragSize(origin: DragOrigin, current: number): number {
  const delta = origin.grow === "forward" ? current - origin.start : origin.start - current;
  return origin.startSize + delta;
}
