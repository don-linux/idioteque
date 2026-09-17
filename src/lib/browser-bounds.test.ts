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
