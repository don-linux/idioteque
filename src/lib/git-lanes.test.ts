import { describe, expect, it } from "vitest";
import { assignLanes, graphWidth, LANE_WIDTH, rowEdges } from "./git-lanes";

describe("assignLanes", () => {
  it("keeps a linear history on a single column", () => {
    const rows = assignLanes([
      { hash: "c", parents: ["b"] },
      { hash: "b", parents: ["a"] },
      { hash: "a", parents: [] },
    ]);

    expect(rows.map((row) => row.column)).toEqual([0, 0, 0]);
    expect(rows.every((row) => !row.merge)).toBe(true);
    expect(rows[0]?.output).toEqual([{ column: 0, id: "b", color: rows[0].color }]);
  });

  it("opens a new lane when a branch forks, then collapses at the shared parent", () => {
    const rows = assignLanes([
      { hash: "d", parents: ["c"] },
      { hash: "b", parents: ["a"] },
      { hash: "c", parents: ["a"] },
      { hash: "a", parents: [] },
    ]);

    expect(rows[0]?.column).toBe(0);
    expect(rows[1]?.column).toBe(1);
    expect(rows[2]?.column).toBe(0);
    expect(rows[3]?.column).toBe(0);
    expect(rows[3]?.input.map((slot) => slot.id).sort()).toEqual(["a", "a"]);
    expect(rows[3]?.output).toEqual([]);
  });

  it("marks a merge and draws a second parent lane", () => {
    const rows = assignLanes([
      { hash: "m", parents: ["c", "d"] },
      { hash: "c", parents: ["a"] },
      { hash: "d", parents: ["a"] },
      { hash: "a", parents: [] },
    ]);

    expect(rows[0]?.merge).toBe(true);
    expect(rows[0]?.column).toBe(0);
    expect(rows[0]?.output.map((slot) => slot.id)).toEqual(["c", "d"]);
    expect(rows[1]?.merge).toBe(false);

    const mergeEdges = rowEdges(rows[0]);
    expect(mergeEdges.some((edge) => edge.d.includes("C "))).toBe(true);
  });

  it("reserves color 0 for the current HEAD commit", () => {
    const rows = assignLanes(
      [
        { hash: "b", parents: ["a"] },
        { hash: "a", parents: [] },
      ],
      "b",
    );

    expect(rows[0]?.color).toBe(0);
    expect(rows[0]?.head).toBe(true);
    expect(rows[1]?.color).toBe(0);
    expect(rows[1]?.head).toBe(false);
  });
});

describe("graphWidth", () => {
  it("grows with the number of active lanes", () => {
    const linear = assignLanes([
      { hash: "b", parents: ["a"] },
      { hash: "a", parents: [] },
    ]);
    const forked = assignLanes([
      { hash: "d", parents: ["c"] },
      { hash: "b", parents: ["a"] },
      { hash: "c", parents: ["a"] },
      { hash: "a", parents: [] },
    ]);

    expect(graphWidth(linear)).toBe(LANE_WIDTH);
    expect(graphWidth(forked)).toBeGreaterThan(graphWidth(linear));
  });
});
