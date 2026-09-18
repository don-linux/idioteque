export interface CssRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PhysicalBounds {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** ADE / X11 geometry is i32. Infinity or 1e40 must not reach JSON. */
const I32_MAX = 2_147_483_647;

function effectiveScale(scale: number): number {
  return Number.isFinite(scale) && scale > 0 ? scale : 1;
}

function finiteOrZero(value: number): number {
  return Number.isFinite(value) ? value : 0;
}

function clampInt(value: number, min: number): number {
  if (!Number.isFinite(value)) return min;
  const rounded = Math.round(value);
  if (!Number.isFinite(rounded)) return min;
  return Math.min(I32_MAX, Math.max(min, rounded));
}

export function physicalBounds(rect: CssRect, scale: number): PhysicalBounds {
  const s = effectiveScale(scale);
  return {
    x: clampInt(finiteOrZero(rect.x) * s, 0),
    y: clampInt(finiteOrZero(rect.y) * s, 0),
    w: clampInt(finiteOrZero(rect.width) * s, 1),
    h: clampInt(finiteOrZero(rect.height) * s, 1),
  };
}

export function cssBoundsOf(rect: CssRect): PhysicalBounds {
  return {
    x: rect.x,
    y: rect.y,
    w: rect.width,
    h: rect.height,
  };
}

export function boundsChanged(
  a: PhysicalBounds | null | undefined,
  b: PhysicalBounds,
): boolean {
  if (!a) return true;
  return a.x !== b.x || a.y !== b.y || a.w !== b.w || a.h !== b.h;
}
