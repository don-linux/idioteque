import { describe, expect, it } from "vitest";
import { tileDimensions, tilePlan, tileRows } from "./terminal-tile";

describe("tileDimensions", () => {
  it("is empty when there are no panes", () => {
    expect(tileDimensions(0, 1200, 800)).toEqual({ cols: 0, rows: 0 });
  });

  it("is a single cell for one pane", () => {
    expect(tileDimensions(1, 1200, 800)).toEqual({ cols: 1, rows: 1 });
    expect(tileDimensions(1, 400, 900)).toEqual({ cols: 1, rows: 1 });
  });

  it("grows the long axis first on a wide canvas", () => {
    expect(tileDimensions(2, 1200, 800)).toEqual({ cols: 2, rows: 1 });
    expect(tileDimensions(3, 1200, 800)).toEqual({ cols: 2, rows: 2 });
    expect(tileDimensions(4, 1200, 800)).toEqual({ cols: 2, rows: 2 });
    expect(tileDimensions(5, 1200, 800)).toEqual({ cols: 3, rows: 2 });
    expect(tileDimensions(6, 1200, 800)).toEqual({ cols: 3, rows: 2 });
  });

  it("grows rows first on a tall canvas", () => {
    expect(tileDimensions(2, 400, 900)).toEqual({ cols: 1, rows: 2 });
    expect(tileDimensions(3, 400, 900)).toEqual({ cols: 2, rows: 2 });
    expect(tileDimensions(4, 400, 900)).toEqual({ cols: 2, rows: 2 });
  });
});

describe("tileRows", () => {
  it("stretches the leftover panes into the last row", () => {
    expect(tileRows(["a", "b", "c"], 2)).toEqual([["a", "b"], ["c"]]);
    expect(tileRows(["a", "b", "c", "d", "e"], 3)).toEqual([
      ["a", "b", "c"],
      ["d", "e"],
    ]);
  });

  it("returns no rows when columns are zero", () => {
    expect(tileRows(["a"], 0)).toEqual([]);
  });
});

describe("tilePlan", () => {
  it("pairs dimensions with row groups", () => {
    expect(tilePlan(["a", "b", "c"], 1200, 800)).toEqual({
      cols: 2,
      rows: 2,
      rowsOfIds: [["a", "b"], ["c"]],
    });
  });
});
