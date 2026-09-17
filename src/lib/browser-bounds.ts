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

export function physicalBounds(rect: CssRect, scale: number): PhysicalBounds {
  return {
    x: Math.max(0, Math.round(rect.x * scale)),
    y: Math.max(0, Math.round(rect.y * scale)),
    w: Math.max(1, Math.round(rect.width * scale)),
    h: Math.max(1, Math.round(rect.height * scale)),
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
