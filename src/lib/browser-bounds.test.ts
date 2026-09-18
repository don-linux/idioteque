import { describe, expect, it } from "vitest";
import { boundsChanged, cssBoundsOf, physicalBounds } from "./browser-bounds";

describe("physicalBounds", () => {
  it("multiplies by scale and rounds", () => {
    expect(
      physicalBounds({ x: 10.4, y: 20.6, width: 100.2, height: 50.8 }, 2),
    ).toEqual({ x: 21, y: 41, w: 200, h: 102 });
  });

  it("clamps negative origin to 0 and sizes to at least 1", () => {
    expect(physicalBounds({ x: -4, y: -0.4, width: 0, height: -8 }, 1)).toEqual({
      x: 0,
      y: 0,
      w: 1,
      h: 1,
    });
  });

  it("treats DPR 0, -0, negative and non-finite scale as 1 so the hole stays put", () => {
    const rect = { x: 400.4, y: 200.6, width: 800.2, height: 600.8 };
    const atOne = physicalBounds(rect, 1);
    expect(atOne).toEqual({ x: 400, y: 201, w: 800, h: 601 });
    expect(physicalBounds(rect, 0)).toEqual(atOne);
    expect(physicalBounds(rect, -0)).toEqual(atOne);
    expect(physicalBounds(rect, -2)).toEqual(atOne);
    expect(physicalBounds(rect, Number.NaN)).toEqual(atOne);
    expect(physicalBounds(rect, Number.POSITIVE_INFINITY)).toEqual(atOne);
    expect(physicalBounds(rect, Number.NEGATIVE_INFINITY)).toEqual(atOne);
  });

  it("keeps fractional desktop DPRs used on Linux (125% / 150% / 175%)", () => {
    const rect = { x: 10, y: 20, width: 100, height: 80 };
    expect(physicalBounds(rect, 1.25)).toEqual({ x: 13, y: 25, w: 125, h: 100 });
    expect(physicalBounds(rect, 1.5)).toEqual({ x: 15, y: 30, w: 150, h: 120 });
    expect(physicalBounds(rect, 1.75)).toEqual({ x: 18, y: 35, w: 175, h: 140 });
  });

  it("keeps a tiny positive scale (0.5 is not DPR 0)", () => {
    expect(physicalBounds({ x: 10, y: 10, width: 100, height: 80 }, 0.5)).toEqual({
      x: 5,
      y: 5,
      w: 50,
      h: 40,
    });
  });

  it("never emits a non-finite or zero-area physical box", () => {
    const box = physicalBounds(
      { x: Number.NaN, y: Number.POSITIVE_INFINITY, width: Number.NaN, height: -Infinity },
      2,
    );
    expect(Object.values(box).every(Number.isFinite)).toBe(true);
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.y).toBeGreaterThanOrEqual(0);
    expect(box.w).toBeGreaterThanOrEqual(1);
    expect(box.h).toBeGreaterThanOrEqual(1);
    expect(boundsChanged(physicalBounds(
      { x: Number.NaN, y: Number.POSITIVE_INFINITY, width: Number.NaN, height: -Infinity },
      2,
    ), box)).toBe(false);
  });

  it("clamps CSS × scale that overflows signed 32-bit (ADE bounds are i32)", () => {
    const box = physicalBounds({ x: 0, y: 0, width: 1e20, height: 1e20 }, 1e20);
    expect(box.w).toBe(2147483647);
    expect(box.h).toBe(2147483647);
    expect(Number.isSafeInteger(box.w)).toBe(true);
  });
});

describe("cssBoundsOf", () => {
  it("renames width/height to w/h without scaling", () => {
    expect(cssBoundsOf({ x: 1, y: 2, width: 3, height: 4 })).toEqual({
      x: 1,
      y: 2,
      w: 3,
      h: 4,
    });
  });

  it("does not clamp a zero CSS box (BrowserView drops width/height < 1)", () => {
    expect(cssBoundsOf({ x: 0, y: 0, width: 0, height: 0 })).toEqual({
      x: 0,
      y: 0,
      w: 0,
      h: 0,
    });
  });
});

describe("boundsChanged", () => {
  const box = { x: 1, y: 2, w: 3, h: 4 };

  it("is true when the previous box is missing", () => {
    expect(boundsChanged(null, box)).toBe(true);
    expect(boundsChanged(undefined, box)).toBe(true);
  });

  it("is false when every field matches", () => {
    expect(boundsChanged({ ...box }, box)).toBe(false);
  });

  it("is true when any field differs", () => {
    expect(boundsChanged({ ...box, x: 0 }, box)).toBe(true);
    expect(boundsChanged({ ...box, y: 0 }, box)).toBe(true);
    expect(boundsChanged({ ...box, w: 9 }, box)).toBe(true);
    expect(boundsChanged({ ...box, h: 5 }, box)).toBe(true);
  });
});
